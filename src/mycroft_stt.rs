use std::io::BufWriter;
use std::io::Write;

use serde_json;

extern crate bitvec;
use bitvec::prelude::BitVec;
use bitvec::prelude::Msb0;

extern crate flacenc;
use flacenc::coding;
use flacenc::component::BitRepr;
use flacenc::component::Stream;
use flacenc::source::PreloadedSignal;

extern crate reqwest;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, AUTHORIZATION};

extern crate rustcroft;
use rustcroft::identity::Identity;

fn write_stream<F: Write>(stream: &Stream, file: &mut F) {
    eprintln!("{} bits to be written", stream.count_bits());
    let mut bv: BitVec<u8, Msb0> = BitVec::with_capacity(stream.count_bits());
    stream.write(&mut bv).expect("Bitstream formatting failed.");
    let mut writer = BufWriter::new(file);
    writer
        .write_all(bv.as_raw_slice())
        .expect("File write failed.");
}

fn convert_to_flac_data(audio_data: Vec<i16>) -> Vec<u8> {
    let mut i32data = vec!();
    for sample in audio_data.iter() {
        i32data.push(*sample as i32);
    }
    let signal = PreloadedSignal::from_samples(
        &i32data[..], 1, 16, 16000);

    let encoder_config = flacenc::config::Encoder::default();
    let stream = if encoder_config.block_sizes.len() == 1 {
        let block_size = encoder_config.block_sizes[0];
        coding::encode_with_fixed_block_size(&encoder_config, signal, block_size)
                                            .expect("Read error.")
    } else {
        coding::encode_with_multiple_block_sizes(&encoder_config, signal)
            .expect("Read error.")
    };

    let mut buffer = Vec::<u8>::new();
    write_stream(&stream, &mut buffer);
    buffer
}

fn create_stt_headers() -> HeaderMap {
    let identity = Identity::load().unwrap();
    println!("Auth: {}", identity.access);
    let mut headers = HeaderMap::new();

    let mut bearer_auth = String::from("Bearer ");
    bearer_auth.push_str(identity.access.as_str());
    headers.insert(AUTHORIZATION, HeaderValue::from_str(bearer_auth.as_str()).unwrap());
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("audio/x-flac"));
    headers
}

pub fn mycroft_stt(audio_data: Vec<i16>) -> Option<Vec<String>> {
    let flac_data = convert_to_flac_data(audio_data);
    let client = reqwest::blocking::Client::new();
    let headers = create_stt_headers();
    
    let response = client.post("https://api.mycroft.ai/v1/stt")
        .headers(headers)
        .query(&[("lang", "en-US"), ("limit", "1")])
        .body(flac_data)
        .send().unwrap();
    if let Ok(sentences) = serde_json::from_str(&response.text().unwrap()) {
        Some(sentences)
    } else {
        println!("Could not parse server response :(");
        None
    }
}
