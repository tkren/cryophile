// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use permafrust;
use std::fs;
use std::fs::File;
use std::io;
use std::io::IoSlice;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use tempfile;

#[test]
fn test_split_write() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let tmp_path = PathBuf::from(tmp_dir.path());
    let out_path = tmp_path.join("out");
    let _ = fs::create_dir(&out_path);
    println!("tmpdir {tmp_path:?}");
    let mut splitter = permafrust::core::Split::new(&tmp_path, &out_path, "chunk", 3);

    let mut s = String::from("0123456789abcdef");

    match splitter.write(s.as_bytes()) {
        Ok(n) => {
            assert_eq!(n, s.len());
        }
        Err(err) => {
            assert!(0 == 1, "Split::write: {err}");
        }
    }

    for i in 1..6 {
        let chunk = format!("chunk.{i}");
        let path = tmp_dir.path().join(&chunk);
        let out_path = out_path.join(&chunk);
        File::open(path.clone()).expect_err("chunk file is still present");
        let mut out_file = File::open(out_path.clone()).expect("failed to open out file");
        let mut buf = String::new();

        out_file
            .read_to_string(&mut buf)
            .expect("failed to read out file");

        let remainder = if s.len() >= 3 {
            s.split_off(3)
        } else {
            s.clone()
        };
        assert_eq!(buf, s);
        s = remainder;

        std::fs::remove_file(path).expect_err("old chunk is still present");
    }

    s.clear();

    match splitter.write(b"") {
        Ok(n) => {
            assert_eq!(n, 0);
        }
        Err(err) => {
            assert!(0 == 1, "Split::write: {err}");
        }
    }

    File::open(tmp_dir.path().join("chunk.1")).expect_err("found chunk file");
}

#[test]
fn test_split_write_vectored() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let tmp_path = PathBuf::from(tmp_dir.path());
    let out_path = tmp_path.join("out");
    let _ = fs::create_dir(&out_path);
    println!("tmpdir {tmp_path:?}");
    let mut splitter = permafrust::core::Split::new(&tmp_path, &out_path, "chunk", 3);

    let s = String::from("0123456789abcdef");

    let io_slices = [IoSlice::new(s.as_bytes()), IoSlice::new(s.as_bytes())];

    match splitter.write_vectored(&io_slices) {
        Ok(n) => {
            assert_eq!(n, 2 * s.len());
        }
        Err(err) => {
            assert!(0 == 1, "Split::write_vectored: {err}");
        }
    }
}

#[test]
fn test_copy_to_split() {
    let tmp_dir = tempfile::TempDir::new().unwrap();
    let tmp_path = PathBuf::from(tmp_dir.path());
    let out_path = tmp_path.join("out");
    let _ = fs::create_dir(&out_path);
    println!("tmpdir {tmp_path:?}");

    let mut splitter = permafrust::core::Split::new(&tmp_path, &out_path, "chunk", 512);

    let s = String::from("0123456789abcdef");
    let strings = s.repeat(1000);
    let mut slice = strings.as_bytes();

    let n = io::copy(&mut slice, &mut splitter).expect("IO error");
    assert_eq!(splitter.written(), n);
}
