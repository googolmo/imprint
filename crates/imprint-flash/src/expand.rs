//! Grow the last MBR/GPT partition so a smaller image fills a larger disk.
//!
//! Only the partition table is updated. The guest OS typically expands the
//! filesystem on first boot (`resize2fs`, systemd-growfs, cloud-init, …).

use std::io::{self, Read, Seek, SeekFrom, Write};

use imprint_core::{Error, Result};
use tracing::info;

use crate::aligned::AlignedIo;

const SECTOR: u64 = 512;
const MBR_SIGNATURE: [u8; 2] = [0x55, 0xAA];
const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";
const ISO_PVD: u64 = 0x8001;
const MIN_GROW: u64 = 1024 * 1024;
const EXTENDED: &[u8] = &[0x05, 0x0f];
const PROTECTIVE: u8 = 0xee;
const MAX_GPT_TABLE: usize = 1024 * 1024;

/// Bytes added to the last partition, or `0` if nothing changed.
pub fn apply_on<T: Read + Write + Seek>(
  dev: &mut T,
  device_bytes: u64,
  sector: usize,
) -> Result<u64> {
  if device_bytes < MIN_GROW * 2 {
    return Ok(0);
  }
  let mut io = AlignedIo::new(dev, sector);
  let added = expand_inner(&mut io, device_bytes, sector)?;
  io.flush().map_err(|err| Error::Expand(err.to_string()))?;
  Ok(added)
}

fn expand_inner<T: Read + Write + Seek>(
  dev: &mut T,
  device_bytes: u64,
  io_sector: usize,
) -> Result<u64> {
  if is_iso9660(dev) {
    info!("partition expand skipped: ISO 9660 image");
    return Ok(0);
  }

  let mut mbr = [0u8; 512];
  read_at(dev, 0, &mut mbr)?;

  if let Some(lba) = gpt_lba_size(dev, io_sector) {
    return expand_gpt(dev, device_bytes, lba);
  }
  if mbr[510..] == MBR_SIGNATURE {
    return expand_mbr(dev, &mut mbr, device_bytes);
  }
  info!("partition expand skipped: no MBR or GPT");
  Ok(0)
}

fn expand_mbr<T: Write + Seek>(dev: &mut T, mbr: &mut [u8; 512], device_bytes: u64) -> Result<u64> {
  let disk_lbas = device_bytes / SECTOR;
  let cap_lbas = disk_lbas.min(u32::MAX as u64);

  let mut best: Option<(usize, u64, u64)> = None;
  for i in 0..4 {
    let entry = &mbr[446 + i * 16..446 + (i + 1) * 16];
    let kind = entry[4];
    if kind == 0 || kind == PROTECTIVE || EXTENDED.contains(&kind) {
      continue;
    }
    let start = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
    let sectors = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as u64;
    if start == 0 || sectors == 0 {
      continue;
    }
    let end = start + sectors;
    if best.map(|(_, s, n)| s + n).unwrap_or(0) <= end {
      best = Some((i, start, sectors));
    }
  }
  let Some((i, start, sectors)) = best else {
    info!("partition expand skipped: no growable MBR partition");
    return Ok(0);
  };
  if start >= cap_lbas {
    return Ok(0);
  }
  let new_sectors = cap_lbas - start;
  if new_sectors <= sectors {
    return Ok(0);
  }
  let added = (new_sectors - sectors) * SECTOR;
  if added < MIN_GROW {
    return Ok(0);
  }

  let entry = &mut mbr[446 + i * 16..446 + (i + 1) * 16];
  entry[12..16].copy_from_slice(&(new_sectors as u32).to_le_bytes());
  // End CHS sentinel used with LBA addressing.
  entry[5] = 0xFE;
  entry[6] = 0xFF;
  entry[7] = 0xFF;
  write_at(dev, 0, mbr)?;
  info!(
    "grew MBR partition {i} by {} bytes ({sectors} → {new_sectors} sectors)",
    added
  );
  Ok(added)
}

fn expand_gpt<T: Read + Write + Seek>(dev: &mut T, device_bytes: u64, lba: u64) -> Result<u64> {
  let disk_lbas = device_bytes / lba;
  if disk_lbas < 64 {
    return Ok(0);
  }

  let mut header = vec![0u8; lba as usize];
  read_at(dev, lba, &mut header)?;
  if header.len() < 92 || &header[..8] != GPT_SIGNATURE {
    return Ok(0);
  }

  let header_size = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
  if header_size < 92 || header_size > lba as usize {
    return Err(Error::Expand("invalid GPT header size".into()));
  }

  let entry_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
  let entry_count = u32::from_le_bytes(header[80..84].try_into().unwrap());
  let entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap());
  if entry_size < 48 || entry_count == 0 {
    return Ok(0);
  }
  let table_len = (entry_count as usize).saturating_mul(entry_size as usize);
  if table_len == 0 || table_len > MAX_GPT_TABLE {
    return Err(Error::Expand("GPT partition array is too large".into()));
  }
  let array_lbas = (table_len as u64).div_ceil(lba);
  let new_backup_lba = disk_lbas - 1;
  let new_backup_array_lba = new_backup_lba.saturating_sub(array_lbas);
  if new_backup_array_lba <= entry_lba {
    return Ok(0);
  }
  let new_last_usable = new_backup_array_lba - 1;
  let first_usable = u64::from_le_bytes(header[40..48].try_into().unwrap());
  if new_last_usable < first_usable {
    return Ok(0);
  }

  let mut table = vec![0u8; table_len];
  read_at(dev, entry_lba * lba, &mut table)?;

  let Some((idx, first, last)) = last_gpt_partition(&table, entry_size as usize) else {
    info!("partition expand skipped: no GPT partitions");
    return Ok(0);
  };
  if first > new_last_usable {
    return Err(Error::Expand(
      "last GPT partition starts past the disk".into(),
    ));
  }

  let mut new_last = last.min(new_last_usable);
  let mut added = 0u64;
  if new_last_usable > last {
    let extra = (new_last_usable - last) * lba;
    if extra >= MIN_GROW {
      new_last = new_last_usable;
      added = extra;
    }
  }

  let old_backup = u64::from_le_bytes(header[32..40].try_into().unwrap());
  if added == 0 && old_backup == new_backup_lba && last == new_last {
    return Ok(0);
  }

  let entry = &mut table[idx..idx + entry_size as usize];
  entry[40..48].copy_from_slice(&new_last.to_le_bytes());
  let array_crc = crc32(&table);

  let disk_guid: [u8; 16] = header[56..72].try_into().unwrap();
  let mut primary = header.clone();
  fill_gpt_header(
    &mut primary,
    header_size,
    1,
    new_backup_lba,
    first_usable,
    new_last_usable,
    disk_guid,
    entry_lba,
    entry_count,
    entry_size,
    array_crc,
  );

  let mut backup = header.clone();
  fill_gpt_header(
    &mut backup,
    header_size,
    new_backup_lba,
    1,
    first_usable,
    new_last_usable,
    disk_guid,
    new_backup_array_lba,
    entry_count,
    entry_size,
    array_crc,
  );

  write_at(dev, new_backup_array_lba * lba, &table)?;
  write_at(dev, new_backup_lba * lba, &backup)?;
  write_at(dev, entry_lba * lba, &table)?;
  write_at(dev, lba, &primary)?;
  patch_protective_mbr(dev, disk_lbas)?;

  info!(
    "grew GPT partition by {added} bytes (last LBA {last} → {new_last}); backup at LBA {new_backup_lba}"
  );
  Ok(added)
}

fn last_gpt_partition(table: &[u8], entry_size: usize) -> Option<(usize, u64, u64)> {
  let mut best: Option<(usize, u64, u64)> = None;
  for (i, chunk) in table.chunks(entry_size).enumerate() {
    if chunk.len() < 48 || chunk[..16].iter().all(|&b| b == 0) {
      continue;
    }
    let first = u64::from_le_bytes(chunk[32..40].try_into().ok()?);
    let last = u64::from_le_bytes(chunk[40..48].try_into().ok()?);
    if last < first {
      continue;
    }
    if best.map(|(_, _, l)| l).unwrap_or(0) <= last {
      best = Some((i * entry_size, first, last));
    }
  }
  best
}

#[allow(clippy::too_many_arguments)]
fn fill_gpt_header(
  header: &mut [u8],
  header_size: usize,
  my_lba: u64,
  alt_lba: u64,
  first_usable: u64,
  last_usable: u64,
  disk_guid: [u8; 16],
  entry_lba: u64,
  entry_count: u32,
  entry_size: u32,
  array_crc: u32,
) {
  header[..8].copy_from_slice(GPT_SIGNATURE);
  header[8..12].copy_from_slice(&0x0001_0000u32.to_le_bytes());
  header[12..16].copy_from_slice(&(header_size as u32).to_le_bytes());
  header[16..20].fill(0);
  header[24..32].copy_from_slice(&my_lba.to_le_bytes());
  header[32..40].copy_from_slice(&alt_lba.to_le_bytes());
  header[40..48].copy_from_slice(&first_usable.to_le_bytes());
  header[48..56].copy_from_slice(&last_usable.to_le_bytes());
  header[56..72].copy_from_slice(&disk_guid);
  header[72..80].copy_from_slice(&entry_lba.to_le_bytes());
  header[80..84].copy_from_slice(&entry_count.to_le_bytes());
  header[84..88].copy_from_slice(&entry_size.to_le_bytes());
  header[88..92].copy_from_slice(&array_crc.to_le_bytes());
  let crc = crc32(&header[..header_size]);
  header[16..20].copy_from_slice(&crc.to_le_bytes());
}

fn patch_protective_mbr<T: Read + Write + Seek>(dev: &mut T, disk_lbas: u64) -> Result<()> {
  let mut mbr = [0u8; 512];
  read_at(dev, 0, &mut mbr)?;
  if mbr[510..] != MBR_SIGNATURE {
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
  }
  for i in 0..4 {
    let entry = &mut mbr[446 + i * 16..446 + (i + 1) * 16];
    if entry[4] != PROTECTIVE {
      continue;
    }
    let start = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
    let size = disk_lbas.saturating_sub(start).min(u32::MAX as u64) as u32;
    entry[12..16].copy_from_slice(&size.to_le_bytes());
    entry[5] = 0xFE;
    entry[6] = 0xFF;
    entry[7] = 0xFF;
  }
  write_at(dev, 0, &mbr)?;
  Ok(())
}

fn gpt_lba_size<T: Read + Seek>(dev: &mut T, io_sector: usize) -> Option<u64> {
  if read_signature(dev, SECTOR) {
    return Some(SECTOR);
  }
  let other = io_sector as u64;
  if other != SECTOR && read_signature(dev, other) {
    return Some(other);
  }
  None
}

fn read_signature<T: Read + Seek>(dev: &mut T, off: u64) -> bool {
  let mut sig = [0u8; 8];
  read_at(dev, off, &mut sig).is_ok() && sig == *GPT_SIGNATURE
}

fn is_iso9660<T: Read + Seek>(dev: &mut T) -> bool {
  let mut sig = [0u8; 5];
  read_at(dev, ISO_PVD, &mut sig).is_ok() && &sig == b"CD001"
}

fn read_at<T: Read + Seek>(dev: &mut T, off: u64, buf: &mut [u8]) -> io::Result<()> {
  dev.seek(SeekFrom::Start(off))?;
  dev.read_exact(buf)
}

fn write_at<T: Write + Seek>(dev: &mut T, off: u64, buf: &[u8]) -> io::Result<()> {
  dev.seek(SeekFrom::Start(off))?;
  dev.write_all(buf)
}

/// CRC-32 (ISO 3309 / ITU-T V.42), as used by EFI GPT.
fn crc32(data: &[u8]) -> u32 {
  let mut crc = 0xFFFF_FFFFu32;
  for &b in data {
    crc ^= u32::from(b);
    for _ in 0..8 {
      crc = if crc & 1 != 0 {
        (crc >> 1) ^ 0xEDB8_8320
      } else {
        crc >> 1
      };
    }
  }
  !crc
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Cursor;

  const ENTRIES: u32 = 128;
  const ENTRY_SIZE: u32 = 128;
  const ARRAY_BYTES: u64 = ENTRIES as u64 * ENTRY_SIZE as u64;
  const ARRAY_LBAS: u64 = ARRAY_BYTES / SECTOR;

  fn mbr_disk(image_bytes: u64, part_start: u64, part_sectors: u64) -> Vec<u8> {
    let mut disk = vec![0u8; image_bytes as usize];
    disk[510] = 0x55;
    disk[511] = 0xAA;
    disk[446 + 4] = 0x0c;
    disk[446 + 8..446 + 12].copy_from_slice(&(part_start as u32).to_le_bytes());
    disk[446 + 12..446 + 16].copy_from_slice(&(part_sectors as u32).to_le_bytes());
    disk
  }

  fn mbr_part(disk: &[u8]) -> (u32, u32) {
    let start = u32::from_le_bytes(disk[446 + 8..446 + 12].try_into().unwrap());
    let sectors = u32::from_le_bytes(disk[446 + 12..446 + 16].try_into().unwrap());
    (start, sectors)
  }

  fn gpt_geometry(disk_bytes: u64) -> (u64, u64, u64) {
    let disk_lbas = disk_bytes / SECTOR;
    let backup = disk_lbas - 1;
    let backup_array = backup - ARRAY_LBAS;
    (disk_lbas, backup_array - 1, backup)
  }

  fn gpt_disk(image_bytes: u64, part_first: u64, part_last: u64) -> Vec<u8> {
    let (disk_lbas, last_usable, backup) = gpt_geometry(image_bytes);
    let mut disk = vec![0u8; image_bytes as usize];

    disk[510] = 0x55;
    disk[511] = 0xAA;
    disk[446 + 4] = PROTECTIVE;
    disk[446 + 8..446 + 12].copy_from_slice(&1u32.to_le_bytes());
    disk[446 + 12..446 + 16].copy_from_slice(&((disk_lbas - 1) as u32).to_le_bytes());

    let mut table = vec![0u8; ARRAY_BYTES as usize];
    table[..16].copy_from_slice(&[1u8; 16]);
    table[16..32].copy_from_slice(&[2u8; 16]);
    table[32..40].copy_from_slice(&part_first.to_le_bytes());
    table[40..48].copy_from_slice(&part_last.to_le_bytes());
    let array_crc = crc32(&table);

    let mut primary = vec![0u8; SECTOR as usize];
    fill_gpt_header(
      &mut primary,
      92,
      1,
      backup,
      34,
      last_usable,
      [0x11; 16],
      2,
      ENTRIES,
      ENTRY_SIZE,
      array_crc,
    );
    let mut secondary = vec![0u8; SECTOR as usize];
    fill_gpt_header(
      &mut secondary,
      92,
      backup,
      1,
      34,
      last_usable,
      [0x11; 16],
      backup - ARRAY_LBAS,
      ENTRIES,
      ENTRY_SIZE,
      array_crc,
    );

    disk[SECTOR as usize..SECTOR as usize * 2].copy_from_slice(&primary);
    disk[SECTOR as usize * 2..SECTOR as usize * 2 + ARRAY_BYTES as usize].copy_from_slice(&table);
    let backup_array = (backup - ARRAY_LBAS) as usize * SECTOR as usize;
    disk[backup_array..backup_array + ARRAY_BYTES as usize].copy_from_slice(&table);
    disk[backup as usize * SECTOR as usize..(backup as usize + 1) * SECTOR as usize]
      .copy_from_slice(&secondary);
    disk
  }

  fn grow_into(image: Vec<u8>, device_bytes: u64) -> Vec<u8> {
    let mut device = vec![0u8; device_bytes as usize];
    device[..image.len()].copy_from_slice(&image);
    let added = apply_on(&mut Cursor::new(&mut device), device_bytes, 512).unwrap();
    assert!(added >= MIN_GROW, "expected a grow, got {added}");
    device
  }

  #[test]
  fn mbr_last_partition_fills_the_device() {
    let start = 2048u64;
    let image_sectors = 2048u64;
    let image = mbr_disk(2 * 1024 * 1024, start, image_sectors);
    let device = grow_into(image, 4 * 1024 * 1024);
    let (got_start, got_sectors) = mbr_part(&device);
    assert_eq!(got_start, start as u32);
    assert_eq!(got_sectors, (4 * 1024 * 1024 / 512 - start) as u32);
  }

  #[test]
  fn gpt_last_partition_and_backup_move_to_new_end() {
    let image_bytes = 2 * 1024 * 1024;
    let device_bytes = 4 * 1024 * 1024;
    let (_, old_last, _) = gpt_geometry(image_bytes);
    let image = gpt_disk(image_bytes, 34, old_last);
    let device = grow_into(image, device_bytes);

    let (_, new_last, new_backup) = gpt_geometry(device_bytes);
    let mut header = [0u8; 92];
    header.copy_from_slice(&device[SECTOR as usize..SECTOR as usize + 92]);
    assert_eq!(&header[..8], GPT_SIGNATURE);
    let crc_stored = u32::from_le_bytes(header[16..20].try_into().unwrap());
    header[16..20].fill(0);
    assert_eq!(crc32(&header), crc_stored);
    assert_eq!(
      u64::from_le_bytes(header[32..40].try_into().unwrap()),
      new_backup
    );
    assert_eq!(
      u64::from_le_bytes(header[48..56].try_into().unwrap()),
      new_last
    );

    let part_last = u64::from_le_bytes(
      device[SECTOR as usize * 2 + 40..SECTOR as usize * 2 + 48]
        .try_into()
        .unwrap(),
    );
    assert_eq!(part_last, new_last);

    let backup_off = new_backup as usize * SECTOR as usize;
    assert_eq!(&device[backup_off..backup_off + 8], GPT_SIGNATURE);
  }

  #[test]
  fn iso9660_is_left_alone() {
    let mut image = mbr_disk(2 * 1024 * 1024, 2048, 2048);
    image[0x8001..0x8006].copy_from_slice(b"CD001");
    let mut device = vec![0u8; 4 * 1024 * 1024];
    device[..image.len()].copy_from_slice(&image);
    let added = apply_on(&mut Cursor::new(&mut device), 4 * 1024 * 1024, 512).unwrap();
    assert_eq!(added, 0);
    assert_eq!(mbr_part(&device), (2048, 2048));
  }

  #[test]
  fn already_full_mbr_is_a_noop() {
    let bytes = 4 * 1024 * 1024;
    let start = 2048u64;
    let sectors = bytes / 512 - start;
    let mut disk = mbr_disk(bytes, start, sectors);
    let added = apply_on(&mut Cursor::new(&mut disk), bytes, 512).unwrap();
    assert_eq!(added, 0);
    assert_eq!(mbr_part(&disk), (start as u32, sectors as u32));
  }

  #[test]
  fn extended_mbr_partition_is_not_grown() {
    let mut image = vec![0u8; 2 * 1024 * 1024];
    image[510] = 0x55;
    image[511] = 0xAA;
    image[446 + 4] = 0x0f;
    image[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
    image[446 + 12..446 + 16].copy_from_slice(&2048u32.to_le_bytes());
    let mut device = vec![0u8; 4 * 1024 * 1024];
    device[..image.len()].copy_from_slice(&image);
    let added = apply_on(&mut Cursor::new(&mut device), 4 * 1024 * 1024, 512).unwrap();
    assert_eq!(added, 0);
  }

  #[test]
  fn crc32_matches_itu_t_vector() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
  }
}
