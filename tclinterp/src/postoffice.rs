use std::{
    io::{self, Read, Write},
    mem,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LetterCommand {
    Eval = b'E',
}

/// Represents a letter that can be passed between post offices.
///
/// A letter consists of a one-byte command and an arbitrary amount of bytes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Letter {
    cmd: LetterCommand,
    data: Vec<u8>,
}

impl Letter {
    pub fn new(cmd: LetterCommand, data: impl Into<Vec<u8>>) -> Self {
        Self {
            cmd,
            data: data.into(),
        }
    }

    pub fn cmd(&self) -> LetterCommand {
        self.cmd
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// A trait for objects which can send an recieve messages. Any (Read + Write) implements
/// PostOffice.
pub trait PostOffice: Read + Write {
    fn send_msg(&mut self, msg: Letter) -> io::Result<()> {
        self.write_all(&[msg.cmd as u8])?;
        self.write_all(&msg.data.len().to_be_bytes())?;
        self.write_all(&msg.data)?;
        self.flush()?;

        Ok(())
    }

    fn recv_msg(&mut self) -> io::Result<Letter> {
        let mut cmd: [u8; 1] = Default::default();
        let mut data_len: [u8; 8] = Default::default();

        self.read_exact(&mut cmd)?;
        self.read_exact(&mut data_len)?;

        let cmd = unsafe { mem::transmute(cmd[0]) };
        let data_len = usize::from_be_bytes(data_len);

        let mut data = vec![0u8; data_len];
        self.read_exact(&mut data)?;

        Ok(Letter { cmd, data })
    }
}

impl<T: Read + Write> PostOffice for T {}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{Cursor, Seek, SeekFrom};

    #[test]
    fn test_postoffice() {
        let mut cursor = Cursor::new(Vec::new());

        let msg = Letter::new(LetterCommand::Eval, "format %s 4\02");
        cursor.send_msg(msg.clone()).unwrap();
        cursor.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(cursor.recv_msg().unwrap(), msg);
    }
}
