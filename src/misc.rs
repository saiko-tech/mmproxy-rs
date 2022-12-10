use std::fs::File;
use std::io::{self, Read};
use std::str::FromStr;

use cidr::IpCidr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Tcp
    }
}

pub fn parse_allowed_subnets(path: &str) -> io::Result<Vec<IpCidr>> {
    let mut ret = Vec::new();
    let mut data = String::new();
    let mut file = File::open(path)?;

    file.read_to_string(&mut data)?;
    for line in data.lines() {
        match IpCidr::from_str(line) {
            Ok(cidr) => ret.push(cidr),
            Err(why) => {
                return Err(io::Error::new(io::ErrorKind::Other, why));
            }
        }
    }

    Ok(ret)
}
