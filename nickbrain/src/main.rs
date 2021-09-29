use std::{collections::HashMap, io::{Read, Write}};

#[derive(Default)]
struct BF {
    mem: HashMap<usize, u8>,
    pos: usize,
}

enum Cmd {
    ToLeft,
    ToRight,
    Read,
    Write,
    Succ,
    Pred,
    Space,
    Cycle(Vec<Cmd>),
}

impl BF {
    fn exec_cmd(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::ToLeft => {
                self.pos -= 1;
            },
            Cmd::ToRight => {
                self.pos += 1;
            },
            Cmd::Read => {
                self.mem.insert(self.pos, std::io::stdin().bytes().next().unwrap().unwrap());
            },
            Cmd::Write => {
                std::io::stdout().write(&[self.mem.get(&self.pos).cloned().unwrap_or_default()]).unwrap();
            },
            Cmd::Succ => {
                let v = self.mem.entry(self.pos).or_default();
                *v = v.wrapping_add(1);
            },
            Cmd::Pred => {
                let v = self.mem.entry(self.pos).or_default();
                *v = v.wrapping_sub(1);
            },
            Cmd::Cycle(data) => {
                while self.mem.get(&self.pos).cloned().unwrap_or_default() != 0 {
                    self.exec(&data);
                }
            },
            Cmd::Space => {}
        }
    }

    fn exec(&mut self, prog: &[Cmd]) {
        for cmd in prog {
            self.exec_cmd(cmd);
        }
    }
}

fn parse_single(inp: &[u8]) -> nom::IResult<&[u8], Cmd> {
    match inp.split_first() {
        Some((b'>', c)) => Ok((c, Cmd::ToRight)),
        Some((b'<', c)) => Ok((c, Cmd::ToLeft)),
        Some((b'+', c)) => Ok((c, Cmd::Succ)),
        Some((b'-', c)) => Ok((c, Cmd::Pred)),
        Some((b'.', c)) => Ok((c, Cmd::Write)),
        Some((b',', c)) => Ok((c, Cmd::Read)),
        Some((b'\n' | b' ', c)) => Ok((c, Cmd::Space)),
        _ => Err(nom::Err::Error(nom::error::make_error(inp, nom::error::ErrorKind::Char)))
    }
}

fn parse_nested(inp: &[u8]) -> nom::IResult<&[u8], Cmd> {
    nom::combinator::map(
        nom::sequence::delimited(nom::bytes::complete::tag("["), nom::multi::many0(parse_single), nom::bytes::complete::tag("]")),
        |v| Cmd::Cycle(v)
    )(inp)
}

fn parse(inp: &[u8]) -> nom::IResult<&[u8], Vec<Cmd>> {
    nom::combinator::all_consuming(nom::multi::many0(nom::branch::alt((parse_single, parse_nested))))(inp)
}

fn main() {
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap();
    let mut bf = BF::default();

    let prog = parse(&mut buf).expect("Failed to parse").1;
    bf.exec(&prog);
}
