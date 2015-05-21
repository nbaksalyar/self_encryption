// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the Safe Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

extern crate self_encryption;
extern crate rand;
extern crate tempdir;

pub use self_encryption::*;
use rand::{thread_rng, Rng};
use std::sync::{Arc,Mutex};

fn random_bytes(length: usize) -> Vec<u8> {
    let mut bytes : Vec<u8> = Vec::with_capacity(length);
    for _ in (0..length) {
        bytes.push(rand::random::<u8>());
    }
    bytes
}

const DATA_SIZE : u64 = 20 * 1024 * 1024;

pub struct Entry {
    name: Vec<u8>,
    data: Vec<u8>
}

pub struct MyStorage {
    entries: Arc<Mutex<Vec<Entry>>>
}

impl MyStorage {
    pub fn new() -> MyStorage {
        MyStorage { entries: Arc::new(Mutex::new(Vec::new())) }
    }

    pub fn has_chunk(&self, name: Vec<u8>) -> bool {
        let lock = self.entries.lock().unwrap();
        for entry in lock.iter() {
            if entry.name == name { return true }
        }
        false
    }
}

impl Storage for MyStorage {
    fn get(&self, name: Vec<u8>) -> Vec<u8> {
        let lock = self.entries.lock().unwrap();
        for entry in lock.iter() {
            if entry.name == name { return entry.data.to_vec() }
        }
        vec![]
    }

    fn put(&self, name: Vec<u8>, data: Vec<u8>) {
        let mut lock = self.entries.lock().unwrap();
        lock.push(Entry { name : name, data : data })
    }
}


#[test]
fn new_read() {
    let read_size : usize = 4096;
    let mut read_position : usize = 0;
    let content_len : usize = 4 * MAX_CHUNK_SIZE as usize;
    let my_storage = Arc::new(MyStorage::new());
    let original = random_bytes(content_len);
    {
        let mut se = SelfEncryptor::new(my_storage.clone(), datamap::DataMap::None);
        se.write(&original, 0);
        {
            let decrypted = se.read(read_position as u64, read_size as u64);
            assert_eq!(original[read_position..(read_position+read_size)].to_vec(),
                       decrypted);

            // read next small part
            read_position += read_size;
            let decrypted = se.read(read_position as u64, read_size as u64);
            assert_eq!(original[read_position ..(read_position+read_size)].to_vec(),
                       decrypted);

            // try to read from end of file, moving the sliding window
            read_position = content_len - 3 * read_size;
            let decrypted = se.read(read_position as u64, read_size as u64);
            assert_eq!(original[read_position ..(read_position+read_size)].to_vec(),
                       decrypted);

            // read again at beginning of file
            read_position = 5usize;
            let decrypted = se.read(read_position as u64, read_size as u64);
            assert_eq!(original[read_position ..(read_position+read_size)].to_vec(),
                       decrypted);

        }

        { // Finish with many small reads
            let mut decrypted : Vec<u8> = Vec::with_capacity(content_len);
            read_position = 0usize;
            for _ in 0..15 {
                decrypted.extend(se.read(read_position as u64, read_size as
                u64).iter().map(|&x| x));
                assert_eq!(original[0..(read_position+read_size)].to_vec(),
                           decrypted);
                read_position += read_size;
            }
        }
        se.close();
    }
}

#[test]
fn write_random_sizes_at_random_positions() {
    let mut rng = thread_rng();
    let my_storage = Arc::new(MyStorage::new());
    let max_broken_size : u64 = 20 * 1024;
    let original = random_bytes(DATA_SIZE as usize);
    // estimate number of broken pieces, not known in advance
    let mut broken_data : Vec<(u64, &[u8])> =
          Vec::with_capacity((DATA_SIZE / max_broken_size) as usize);

    let mut offset : u64 = 0;
    let mut last_piece : u64 = 0;
    while offset < DATA_SIZE {
        let size : u64;
        if DATA_SIZE - offset < max_broken_size {
            size = DATA_SIZE - offset;
            last_piece = offset;
        } else {
            size = rand::random::<u64>() % max_broken_size;
        }
        let piece : (u64, &[u8]) = (offset,
              &original[offset as usize..(offset + size) as usize]);
        broken_data.push(piece);
        offset += size;
    }

    {
        let slice_broken_data = &mut broken_data[..];
        rng.shuffle(slice_broken_data);
    }

    match broken_data.iter()
                     .filter(|&x| x.0 != last_piece)
                     .last() {
        None => panic!("Should never occur. Error in test itself."),
        Some(overlap) => {
            let mut extra : Vec<u8> = overlap.1.to_vec();
            extra.extend(random_bytes(7usize)[..].iter().map(|&x| x));
            let post_overlap : (u64, &[u8]) = (overlap.0, &mut extra[..]);
            let post_position: u64 = overlap.0 + overlap.1.len() as u64;
            let mut wtotal : u64 = 0;

            let mut se = SelfEncryptor::new(my_storage.clone(), datamap::DataMap::None);
            for element in broken_data.iter() {
                se.write(element.1, element.0);
                wtotal += element.1.len() as u64;
            }
            assert_eq!(wtotal, DATA_SIZE);
            let decrypted = se.read(0u64, DATA_SIZE);
            assert_eq!(original, decrypted);

            let mut overwrite = original[0..post_overlap.0 as usize].to_vec();
            overwrite.extend((post_overlap.1).to_vec().iter().map(|&x| x));
            overwrite.extend(original[post_position as usize + 7..DATA_SIZE as
            usize].iter().map(|&x| x));
            se.write(post_overlap.1, post_overlap.0);
            let decrypted = se.read(0u64, DATA_SIZE);
            assert_eq!(overwrite.len(), decrypted.len());
            assert_eq!(overwrite, decrypted);
        }
    }
}

#[test]
// The test writes random-sized pieces at random offsets and checks they can be read back.  The
// pieces may overlap or leave gaps in the file.  Gaps should be filled with 0s when read back.
fn write_random_sizes_out_of_sequence_with_gaps_and_overlaps() {
    let my_storage = Arc::new(MyStorage::new());
    let parts = 20usize;
    assert!(DATA_SIZE / MAX_CHUNK_SIZE as u64 >= parts as u64);
    let mut rng = thread_rng();
    let mut total_size = 0u64;
    let mut data_map = datamap::DataMap::None;
    let mut original = vec![0u8; DATA_SIZE as usize];

    {
        let mut self_encryptor = SelfEncryptor::new(my_storage.clone(), data_map);

        for i in 0..parts {
            // Get random values for the piece size and intended offset
            let piece_size = rng.gen_range(1, MAX_CHUNK_SIZE as usize + 1);
            let offset = rng.gen_range(0, DATA_SIZE - MAX_CHUNK_SIZE as u64);
            total_size = std::cmp::max(total_size, offset + piece_size as u64);
            assert!(DATA_SIZE >= total_size as u64);
            println!("{}\tWriting {} bytes.\tOffset {} bytes.\tTotal size now {} bytes.", i, piece_size,
                     offset, total_size);

            // Create the random piece and copy to the comparison vector.
            let piece = random_bytes(piece_size);
            for a in 0..piece_size {
                original[offset as usize + a] = piece[a];
            }

            // Write the piece to the encryptor and check it can be read back.
            self_encryptor.write(&piece, offset as u64);
            let decrypted = self_encryptor.read(offset, piece_size as u64);
            assert_eq!(decrypted, piece);
            assert_eq!(total_size, self_encryptor.len());
        }

        // Read back DATA_SIZE from the encryptor.  This will contain all that was written, plus likely
        // will be reading past EOF.  Reading past the end shouldn't affect the file size.
        let decrypted = self_encryptor.read(0u64, DATA_SIZE);
        assert_eq!(decrypted.len(), DATA_SIZE as usize);
        assert_eq!(decrypted, original);
        assert_eq!(total_size, self_encryptor.len());

        // Close the encryptor, open a new one with the returned DataMap, and read back DATA_SIZE
        // again.
        data_map = self_encryptor.close();
    }
    
    println!("Reloading data map...");

    let mut self_encryptor = SelfEncryptor::new(my_storage.clone(), data_map);
    let decrypted = self_encryptor.read(0u64, DATA_SIZE);
    assert_eq!(decrypted.len(), DATA_SIZE as usize);
    assert_eq!(decrypted, original);
    assert_eq!(total_size, self_encryptor.len());    
}

#[test]
fn cross_platform_check() {
    let mut chars0 = Vec::<u8>::new();
    let mut chars1 = Vec::<u8>::new();
    let mut chars2 = Vec::<u8>::new();

    // 1Mb of data for each chunk...
    for _ in 0..8192 {
        for j in 0..128 {
            chars0.push(j);
            chars1.push(j);
            chars2.push(j);
        }
    }

    chars1[0] = 1;
    chars2[0] = 2;

    let storage = Arc::new(MyStorage::new());
    let mut data_map = datamap::DataMap::None;

    {
        let mut self_encryptor = SelfEncryptor::new(storage.clone(), data_map);
        self_encryptor.write(&chars0[..], 0);
        self_encryptor.write(&chars1[..], chars0.len() as u64);
        self_encryptor.write(&chars2[..], chars0.len() as u64 + chars1.len() as u64);
        data_map = self_encryptor.close();
    }

    static EXPECTED_HASHES: [[u8; 64]; 3] = [
        [164, 007, 181, 255, 047, 247, 244, 156, 132, 172, 046, 039, 174, 155, 023, 149,
         238, 118, 091, 134, 110, 009, 064, 179, 081, 242, 092, 075, 118, 099, 002, 124,
         143, 167, 155, 140, 069, 010, 196, 053, 177, 078, 213, 171, 206, 119, 043, 214,
         135, 169, 196, 027, 221, 216, 029, 173, 218, 156, 077, 126, 186, 202, 176, 063],
        [128, 020, 139, 037, 223, 045, 197, 035, 116, 013, 162, 251, 227, 001, 186, 118,
         202, 251, 125, 024, 238, 187, 141, 061, 144, 060, 087, 169, 196, 090, 000, 014,
         135, 105, 016, 233, 028, 209, 114, 100, 248, 208, 035, 183, 060, 115, 151, 109,
         126, 119, 216, 003, 092, 221, 035, 022, 065, 188, 241, 104, 213, 105, 171, 132],
        [017, 166, 161, 171, 167, 025, 246, 197, 186, 162, 074, 153, 080, 138, 015, 009,
         223, 017, 167, 184, 193, 226, 102, 181, 045, 086, 230, 221, 150, 128, 108, 152,
         066, 062, 184, 116, 037, 168, 016, 006, 080, 099, 012, 197, 144, 213, 119, 193,
         017, 080, 146, 027, 192, 203, 190, 005, 014, 088, 152, 163, 123, 126, 245, 202]
    ];

    assert_eq!(3, data_map.get_chunks().len());

    let chunks = data_map.get_chunks();

    // for i in 0..chunks.len() {
    //     println!("");
    //     for j in 0..chunks[i].hash.len() {
    //         print!("{}, ", chunks[i].hash[j]);
    //     }
    //     println!("");
    // }

    for i in 0..chunks.len() {
        println!("");
        for j in 0..chunks[i].hash.len() {
            assert_eq!(EXPECTED_HASHES[i][j], chunks[i].hash[j]);
            print!("({},{}) ", EXPECTED_HASHES[i][j], chunks[i].hash[j]);
        }
        println!("");
    }





    // // no compression...
    // static EXPECTED_HASHES: [[u8; 64]; 3] = [
    //     [118, 247, 202, 197, 048, 078, 032, 120, 127, 152, 055, 038, 132, 187, 158, 078,
    //      198, 059, 024, 187, 254, 086, 018, 212, 136, 090, 198, 222, 055, 217, 046, 024,
    //      220, 123, 139, 175, 183, 044, 153, 043, 146, 146, 151, 118, 219, 135, 250, 078,
    //      038, 179, 034, 157, 202, 090, 036, 232, 127, 247, 127, 146, 016, 039, 229, 244],
    //     [173, 011, 204, 080, 163, 184, 016, 069, 230, 252, 046, 250, 071, 173, 159, 077,
    //      123, 099, 057, 220, 227, 249, 186, 000, 231, 107, 199, 216, 181, 243, 029, 180,
    //      179, 218, 115, 243, 046, 067, 004, 049, 173, 228, 214, 202, 114, 149, 215, 128,
    //      170, 170, 107, 034, 249, 168, 255, 012, 167, 147, 085, 034, 075, 063, 008, 025],
    //     [201, 127, 114, 214, 162, 008, 001, 204, 073, 145, 233, 053, 242, 092, 236, 232,
    //      212, 214, 128, 083, 008, 189, 119, 244, 074, 076, 191, 120, 177, 073, 214, 176,
    //      026, 249, 031, 048, 004, 016, 164, 102, 235, 200, 191, 098, 095, 075, 045, 147,
    //      241, 086, 176, 055, 116, 180, 195, 033, 241, 084, 117, 084, 107, 084, 129, 044]
    // ];

    // assert_eq!(3, data_map.get_chunks().len());

    // let chunks = data_map.get_chunks();

    // for i in 0..chunks.len() {
    //     println!("");
    //     for j in 0..chunks[i].hash.len() {
    //         assert_eq!(EXPECTED_HASHES[i][j], chunks[i].hash[j]);
    //         print!("({},{}) ", EXPECTED_HASHES[i][j], chunks[i].hash[j]);
    //     }
    //     println!("");
    // }
}
