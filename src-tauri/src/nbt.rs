use std::io::{self, Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(u8, Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

const MAX_DEPTH: usize = 512;

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Tag> {
        match self {
            Tag::Compound(v) => v.iter_mut().find(|(k, _)| k == key).map(|(_, t)| t),
            _ => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Tag::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Tag::Byte(n) => Some(*n as i64),
            Tag::Short(n) => Some(*n as i64),
            Tag::Int(n) => Some(*n as i64),
            Tag::Long(n) => Some(*n),
            _ => None,
        }
    }
}

struct Reader<R: Read>(R);

impl<R: Read> Reader<R> {
    fn bytes<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut b = [0; N];
        self.0.read_exact(&mut b)?;
        Ok(b)
    }

    fn len(&mut self) -> io::Result<usize> {
        let n = i32::from_be_bytes(self.bytes()?);
        usize::try_from(n).map_err(|_| bad("negative length"))
    }

    fn vec<T>(&mut self, mut one: impl FnMut(&mut Self) -> io::Result<T>) -> io::Result<Vec<T>> {
        let n = self.len()?;
        let mut v = Vec::with_capacity(n.min(4096));
        for _ in 0..n {
            v.push(one(self)?);
        }
        Ok(v)
    }

    fn string(&mut self) -> io::Result<String> {
        let n = u16::from_be_bytes(self.bytes()?) as usize;
        let mut buf = vec![0; n];
        self.0.read_exact(&mut buf)?;
        Ok(decode_mutf8(&buf))
    }

    fn payload(&mut self, id: u8, depth: usize) -> io::Result<Tag> {
        if depth > MAX_DEPTH {
            return Err(bad("nesting too deep"));
        }
        Ok(match id {
            1 => Tag::Byte(i8::from_be_bytes(self.bytes()?)),
            2 => Tag::Short(i16::from_be_bytes(self.bytes()?)),
            3 => Tag::Int(i32::from_be_bytes(self.bytes()?)),
            4 => Tag::Long(i64::from_be_bytes(self.bytes()?)),
            5 => Tag::Float(f32::from_be_bytes(self.bytes()?)),
            6 => Tag::Double(f64::from_be_bytes(self.bytes()?)),
            7 => Tag::ByteArray(self.vec(|r| Ok(i8::from_be_bytes(r.bytes()?)))?),
            8 => Tag::String(self.string()?),
            9 => {
                let [inner] = self.bytes()?;
                Tag::List(inner, self.vec(|r| r.payload(inner, depth + 1))?)
            }
            10 => {
                let mut v = vec![];
                loop {
                    let [t] = self.bytes()?;
                    if t == 0 {
                        break;
                    }
                    let name = self.string()?;
                    v.push((name, self.payload(t, depth + 1)?));
                }
                Tag::Compound(v)
            }
            11 => Tag::IntArray(self.vec(|r| Ok(i32::from_be_bytes(r.bytes()?)))?),
            12 => Tag::LongArray(self.vec(|r| Ok(i64::from_be_bytes(r.bytes()?)))?),
            _ => return Err(bad("unknown tag")),
        })
    }
}

pub fn read(r: impl Read) -> io::Result<Tag> {
    let mut r = Reader(r);
    let [id] = r.bytes()?;
    if id != 10 {
        return Err(bad("root is not a compound"));
    }
    r.string()?;
    r.payload(id, 0)
}

fn put_len(w: &mut impl Write, n: usize) -> io::Result<()> {
    w.write_all(&i32::try_from(n).map_err(|_| bad("too long"))?.to_be_bytes())
}

fn put_string(w: &mut impl Write, s: &str) -> io::Result<()> {
    let b = encode_mutf8(s);
    w.write_all(&u16::try_from(b.len()).map_err(|_| bad("string too long"))?.to_be_bytes())?;
    w.write_all(&b)
}

fn put_payload(w: &mut impl Write, t: &Tag) -> io::Result<()> {
    match t {
        Tag::Byte(n) => w.write_all(&n.to_be_bytes()),
        Tag::Short(n) => w.write_all(&n.to_be_bytes()),
        Tag::Int(n) => w.write_all(&n.to_be_bytes()),
        Tag::Long(n) => w.write_all(&n.to_be_bytes()),
        Tag::Float(n) => w.write_all(&n.to_be_bytes()),
        Tag::Double(n) => w.write_all(&n.to_be_bytes()),
        Tag::ByteArray(v) => {
            put_len(w, v.len())?;
            v.iter().try_for_each(|n| w.write_all(&n.to_be_bytes()))
        }
        Tag::String(s) => put_string(w, s),
        Tag::List(inner, v) => {
            let inner = v.first().map_or(*inner, Tag::id);
            w.write_all(&[inner])?;
            put_len(w, v.len())?;
            v.iter().try_for_each(|t| put_payload(w, t))
        }
        Tag::Compound(v) => {
            for (k, t) in v {
                w.write_all(&[t.id()])?;
                put_string(w, k)?;
                put_payload(w, t)?;
            }
            w.write_all(&[0])
        }
        Tag::IntArray(v) => {
            put_len(w, v.len())?;
            v.iter().try_for_each(|n| w.write_all(&n.to_be_bytes()))
        }
        Tag::LongArray(v) => {
            put_len(w, v.len())?;
            v.iter().try_for_each(|n| w.write_all(&n.to_be_bytes()))
        }
    }
}

pub fn write(w: &mut impl Write, root: &Tag) -> io::Result<()> {
    w.write_all(&[root.id()])?;
    put_string(w, "")?;
    put_payload(w, root)
}

fn decode_mutf8(b: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as u16;
        let (unit, step) = if c < 0x80 {
            (c, 1)
        } else if c & 0xE0 == 0xC0 && i + 1 < b.len() {
            (((c & 0x1F) << 6) | (b[i + 1] as u16 & 0x3F), 2)
        } else if c & 0xF0 == 0xE0 && i + 2 < b.len() {
            (((c & 0x0F) << 12) | ((b[i + 1] as u16 & 0x3F) << 6) | (b[i + 2] as u16 & 0x3F), 3)
        } else {
            (0xFFFD, 1)
        };
        units.push(unit);
        i += step;
    }
    String::from_utf16_lossy(&units)
}

fn encode_mutf8(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for u in s.encode_utf16() {
        match u {
            0x01..=0x7F => out.push(u as u8),
            0x00 | 0x80..=0x7FF => out.extend([0xC0 | (u >> 6) as u8, 0x80 | (u & 0x3F) as u8]),
            _ => out.extend([0xE0 | (u >> 12) as u8, 0x80 | ((u >> 6) & 0x3F) as u8, 0x80 | (u & 0x3F) as u8]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_everything() {
        let root = Tag::Compound(vec![
            (
                "servers".into(),
                Tag::List(10, vec![Tag::Compound(vec![
                    ("name".into(), Tag::String("Café 🎮\0".into())),
                    ("ip".into(), Tag::String("play.example.net".into())),
                    ("acceptTextures".into(), Tag::Byte(1)),
                ])]),
            ),
            ("empty".into(), Tag::List(0, vec![])),
            ("arr".into(), Tag::LongArray(vec![1, -2])),
            ("n".into(), Tag::Double(1.5)),
        ]);
        let mut buf = vec![];
        write(&mut buf, &root).unwrap();
        assert_eq!(read(buf.as_slice()).unwrap(), root);
        assert!(!buf.windows(4).any(|w| w == "🎮".as_bytes()));
    }

    #[test]
    fn rejects_garbage() {
        assert!(read(&[10u8, 0, 0, 9, 0, 1, 9, 0xFF, 0xFF, 0xFF, 0xFF][..]).is_err());
        assert!(read(&[1u8, 0, 0, 5][..]).is_err());
    }
}
