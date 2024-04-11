use std::env::var;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use crate::ipc::client::{Connection, RichClient};
use crate::ipc::utils;

impl Connection for RichClient {
    fn connect(client_id: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let path = var("XDG_RUNTIME_DIR")
            .or_else(|_| var("TMPDIR"))
            .or_else(|_| var("TMP"))
            .or_else(|_| var("TEMP"))
            .unwrap_or_else(|_| "/tmp".to_string());
        for i in 0..10 {
            match UnixStream::connect(format!("{}/discord-ipc-{}", path, i)) {
                Ok(pipe) => {
                    return Ok(RichClient {
                        client_id: client_id,
                        pipe: Some(pipe),
                        last_activity: None,
                    })
                }
                Err(e) => match e.kind() {
                    io::ErrorKind::NotFound => continue,
                    _ => return Err(e.into()),
                },
            }
        }

        Err("Pipe not found".into())
    }

    fn write(&mut self, opcode: u32, data: Option<&[u8]>) -> io::Result<()> {
        if let Some(pipe) = self.pipe.as_mut() {
            if let Some(packet) = data {
                self.stream.write_all(
                    utils::encode(opcode, packet.len() as u32).as_slice(),
                )?;
                self.stream.write_all(packet)?;
            } else {
                self.stream.write_all(utils::encode(opcode, 0).as_slice())?;
            }
        }
        Ok(())
    }

    fn read(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if let Some(pipe) = self.pipe.as_mut() {
            let mut header = [0; 8];
            self.stream.read_exact(&mut header)?;
            let mut buffer = vec![0u8; utils::decode(&header) as usize];
            self.stream.read_exact(&mut buffer)?;
            return Ok(buffer);
        }

        Err("Pipe not found".into())
    }

    fn close(&mut self) -> io::Result<()> {
        if let Some(mut pipe) = self.pipe.take() {
            self.stream.write_all(utils::encode(2, 0).as_slice())?;
            self.stream.flush()?;
        }

        Ok(())
    }

    fn handshake(&mut self) -> io::Result<()> {
        self.write(
            0,
            Some(
                (format!("{{\"v\": 1,\"client_id\":\"{}\"}}", self.client_id))
                    .as_bytes(),
            ),
        )
    }

    fn update(
        &mut self,
        packet: &crate::rpc::packet::Packet,
    ) -> io::Result<()> {
        if packet.activity != self.last_activity {
            return self.write(1, Some(packet.to_json().unwrap().as_bytes()));
        }

        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.write(1, None)
    }
}
