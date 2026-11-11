//! IPv4 CIDR ranges.
//!
//! A dotted quad is not something the ordinary lexer can read: `10.4.64.37` is three
//! numbers separated by dots, and the third separator is a character the language has no
//! use for. A CIDR range is therefore recognized before lexing, as a whole line that is
//! only digits, dots and one slash. Anything else falls through to the calculator, so
//! `10.4 + 2` is still arithmetic and `2.5/2` is still division.
//!
//! The range itself is a value with an address and a prefix. The network, broadcast and
//! first usable addresses are derived from those bits when the value is rendered.

use std::net::Ipv4Addr;

/// An IPv4 address and the length of its network prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    address: Ipv4Addr,
    prefix: u8,
}

/// What [`parse`] made of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attempt {
    /// Not a CIDR range at all, so the calculator should parse it normally.
    NotCidr,
    /// A range that is still being typed.
    Incomplete,
    /// A range that cannot become valid, and why.
    Invalid(String),
    /// A complete range.
    Cidr(Cidr),
}

impl Cidr {
    /// An address and a prefix, if the prefix is one an IPv4 address can have.
    pub fn new(address: Ipv4Addr, prefix: u8) -> Option<Self> {
        (prefix <= 32).then_some(Self { address, prefix })
    }

    /// The same, from the 32 bits of the address, which is how it travels through
    /// Scheme as a number.
    pub fn from_bits(bits: u32, prefix: u8) -> Option<Self> {
        Self::new(Ipv4Addr::from(bits), prefix)
    }

    pub fn address(&self) -> Ipv4Addr {
        self.address
    }

    pub fn prefix(&self) -> u8 {
        self.prefix
    }

    pub fn bits(&self) -> u32 {
        u32::from(self.address)
    }

    fn mask(&self) -> u32 {
        if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix)
        }
    }

    fn network_bits(&self) -> u32 {
        self.bits() & self.mask()
    }

    /// The first address in the range, where every host bit is zero.
    pub fn network(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.network_bits())
    }

    /// The last address in the range, where every host bit is one.
    pub fn broadcast(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.network_bits() | !self.mask())
    }

    /// The second address in the range, which is what networks conventionally use as
    /// their gateway.
    ///
    /// This is a convention and not part of the prefix: a gateway can be any address in
    /// the range, and it is configured rather than derived. A single-address range has
    /// no second address, so there is nothing to suggest.
    pub fn gateway(&self) -> Option<Ipv4Addr> {
        (self.prefix < 32).then(|| Ipv4Addr::from(self.network_bits() + 1))
    }
}

/// Recognize a whole-line IPv4 CIDR query, or say why it is not one yet.
///
/// Whitespace is ignored, the way the rest of the calculator ignores it, so an address
/// and its prefix can arrive with spaces around the slash.
pub fn parse(input: &str) -> Attempt {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let text = compact.as_str();

    // One dot is a decimal number, and two dotted numbers with a slash between them are
    // a division. A dotted quad has at least three dots, and while one is being typed it
    // has at least two.
    if text.matches('.').count() < 2 {
        return Attempt::NotCidr;
    }

    if !text
        .chars()
        .all(|character| character.is_ascii_digit() || character == '.' || character == '/')
    {
        return Attempt::NotCidr;
    }

    let (address, prefix) = match text.split_once('/') {
        Some((_, prefix)) if prefix.contains('/') => {
            return Attempt::Invalid(format!("ugyldig CIDR: {text}"));
        }
        Some((address, prefix)) => (address, Some(prefix)),
        None => (text, None),
    };

    let Some(address) = parse_address(address) else {
        return Attempt::Invalid(format!("ugyldig IP-adresse: {address}"));
    };

    let address = match address {
        Some(address) => address,
        None => return Attempt::Incomplete,
    };

    let prefix = match prefix {
        None => 32,
        Some("") => return Attempt::Incomplete,
        Some(prefix) => match prefix.parse::<u8>() {
            Ok(prefix) if prefix <= 32 => prefix,
            _ => return Attempt::Invalid(format!("ugyldigt CIDR-præfiks: /{prefix}")),
        },
    };

    match Cidr::new(address, prefix) {
        Some(cidr) => Attempt::Cidr(cidr),
        None => Attempt::Invalid(format!("ugyldigt CIDR-præfiks: /{prefix}")),
    }
}

/// Four decimal octets: `None` when the text cannot be an address, `Some(None)` while
/// it is still being typed, and `Some(Some(_))` when it is complete.
fn parse_address(text: &str) -> Option<Option<Ipv4Addr>> {
    let octets: Vec<&str> = text.split('.').collect();

    if octets.len() > 4 {
        return None;
    }

    let mut values = [0u8; 4];

    for (index, octet) in octets.iter().enumerate() {
        if octet.is_empty() {
            // A trailing empty octet is where the next keystroke goes; an empty one
            // anywhere else is a double dot that more typing will not mend.
            return (index + 1 == octets.len()).then_some(None);
        }

        if octet.len() > 3 {
            return None;
        }

        let Ok(value) = octet.parse::<u16>() else {
            return None;
        };

        if value > 255 {
            return None;
        }

        values[index] = value as u8;
    }

    if octets.len() < 4 {
        return Some(None);
    }

    Some(Some(Ipv4Addr::from(values)))
}
