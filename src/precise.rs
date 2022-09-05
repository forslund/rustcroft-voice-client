use std::process::{Command, Child, ChildStdout, ChildStdin, Stdio};
use std::io::Write;
use std::io::BufReader;
use std::io::BufRead;
use std::result::Result;


pub struct PreciseEngine {
    process: Child,
    model: String,
    chunk_size: usize,
    reader: BufReader<ChildStdout>,
    writer: ChildStdin
}

fn parse_confidence_line(line: String) -> Result<bool , &'static str> {
    match line.trim().parse::<f32>() {
        Ok(level) => Ok(level > 0.5),
        Err(e) => {
            println!("Error converting confidence: {}", e);
            Ok(false)
        }
    }
}


impl PreciseEngine {
    #[allow(dead_code)]
    pub fn stop(&mut self) {
        match self.process.kill() {
            Ok(_) => (),
            Err(_) => println!("Couldn't kill process :(")
        }
    }

    #[allow(dead_code)]
    pub fn get_prediction(&mut self, audio_data: &[i16]) -> Result<bool, &'static str> {
        if audio_data.len() % self.chunk_size != 0 {
            Err("audio data length doesn't match the expected chunk size")
        }
        else {
            let bytes: Vec<u8> = bincode::serialize(&audio_data).unwrap();
            let buffer = &bytes[..];
            if let Err(e) = self.writer.write_all(buffer) {
                println!("Error writing to precise ({:?})", e);
            }
            self.writer.flush().unwrap();

            let mut result: bool = false;

            // TODO: Why 4? Find better way to read all output
            for _ in 0..4 {
                let mut output_data: String = "".into();
                self.reader.read_line(&mut output_data);
                if let Ok(is_confident) = parse_confidence_line(output_data) {
                    result |= is_confident;
                }
            }
            Ok(result)
        }
    }

    #[allow(dead_code)]
    fn get_model(&self) -> String {
        self.model.clone()
    }
}

    
pub fn get_runner() -> PreciseEngine {
    
    let cmd = "/home/ake/.mycroft/precise/precise-engine/precise-engine";
    let model = "/home/ake/.mycroft/precise/hey-mycroft.pb";

    let mut child = Command::new(cmd).stdin(Stdio::piped())
        .arg(model)
        .arg("2048")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn().unwrap();

    let out = child.stdout.take().unwrap();
    let cmd_out = BufReader::new(out);

    let cmd_in = child.stdin.take().unwrap();

    PreciseEngine {
        process: child,
        model: model.to_string(),
        chunk_size: 2048,
        reader: cmd_out,
        writer: cmd_in
    }
}
