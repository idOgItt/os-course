#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![allow(dead_code)]
#![allow(unused_imports)]
use anyhow::{bail, Context, Result};
use itertools::Itertools;
use structopt::StructOpt;

use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const ALLOW_DEAD_CODE: &str = r#"#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

"#;

#[derive(StructOpt, Debug, Clone)]
#[structopt()]
struct Opts {
    /// Path to the private repo.
    #[structopt(short = "i", long = "in-path")]
    in_path: PathBuf,

    /// Path to the public repo.
    #[structopt(short = "o", long = "out-path")]
    out_path: Option<PathBuf>,

    /// Spare given directories from pruning.
    #[structopt(short = "s", long = "spare")]
    spare: Vec<PathBuf>,

    #[structopt(long = "stat")]
    stat: bool,
}

enum TokenKind {
    Private,
    Public,
    BeginPrivate,
    EndPrivate,
}

#[derive(Clone, PartialEq, Eq)]
enum TokenProperty {
    Task(String),
    Stub(String),
    NoHint,
    Unimplemented,
}

struct Token {
    kind: TokenKind,
    properties: Vec<TokenProperty>,
}

fn parse_token(line: &str) -> Result<Option<Token>> {
    let comment = match line.find("//") {
        Some(pos) => &line[pos..],
        None => return Ok(None),
    };

    let cmd = match comment.find("compose::") {
        Some(pos) => &comment[pos + "compose::".len()..],
        None => return Ok(None),
    };

    let kind = if cmd.starts_with("private") {
        TokenKind::Private
    } else if cmd.starts_with("public") {
        TokenKind::Public
    } else if cmd.starts_with("begin_private") {
        TokenKind::BeginPrivate
    } else if cmd.starts_with("end_private") {
        TokenKind::EndPrivate
    } else {
        bail!("unknown compose command: {}", cmd);
    };

    let properties_str = match cmd.find('(') {
        Some(pos) => {
            if !cmd.trim_end().ends_with(')') {
                bail!("unclosed '('");
            }
            &cmd[pos + 1..cmd.trim_end().len() - 1]
        },
        None => "",
    };

    let mut properties = vec![];
    match kind {
        TokenKind::Public => properties.push(TokenProperty::Stub(String::from(properties_str))),
        _ => {
            for (i, prop) in properties_str.split(',').enumerate() {
                match prop {
                    "no_hint" => properties.push(TokenProperty::NoHint),
                    "unimplemented" => properties.push(TokenProperty::Unimplemented),
                    s => {
                        if i == 0 {
                            properties.push(TokenProperty::Task(String::from(s)))
                        } else {
                            bail!("unknown property: {}", prop)
                        }
                    },
                }
            }
        },
    }

    Ok(Some(Token { kind, properties }))
}

fn find_token(lines: &[&str], start: usize) -> Result<Option<(usize, Token)>> {
    for (i, line) in lines[start..].iter().enumerate() {
        let mb_token = parse_token(line)
            .with_context(|| format!("failed to parse token on line {}", i + 1))?;
        if let Some(token) = mb_token {
            return Ok(Some((i + start, token)));
        }
    }
    Ok(None)
}

struct ProcessedSource {
    src: String,

    removed_lines: usize,
    removed_by_task: HashMap<String, usize>,
}

fn process_source(in_path: &Path, src: String) -> Result<ProcessedSource> {
    let mut dst = String::new();
    let mut removed_lines: usize = 0;
    let mut removed_by_task: HashMap<String, usize> = HashMap::new();

    if in_path.ends_with("src/main.rs") || in_path.ends_with("src/lib.rs") {
        dst += ALLOW_DEAD_CODE;
    }

    let lines = src.lines().collect::<Vec<_>>();
    let mut next_pos = 0;
    while let Some((begin, token)) = find_token(&lines, next_pos)? {
        let end = match token.kind {
            TokenKind::EndPrivate => bail!("unpaired 'end_private' on line {}", begin + 1),
            TokenKind::Private | TokenKind::Public => begin + 1,
            TokenKind::BeginPrivate => {
                let mut pos = begin + 1;
                let mut mb_end: Option<usize> = None;
                while let Some((k, token)) = find_token(&lines, pos)? {
                    match token.kind {
                        TokenKind::BeginPrivate => {
                            bail!("nested 'begin_private' on line {}", k + 1)
                        },
                        TokenKind::Private | TokenKind::Public => pos = k + 1,
                        TokenKind::EndPrivate => {
                            mb_end = Some(k);
                            break;
                        },
                    }
                }
                match mb_end {
                    Some(end) => end + 1,
                    None => bail!("unclosed 'begin_private' on line {}", begin + 1),
                }
            },
        };

        for line in lines.iter().take(begin).skip(next_pos) {
            dst += line;
            dst += "\n";
        }

        removed_lines += end - begin;
        if !token.properties.is_empty() {
            if let TokenProperty::Task(task) = &token.properties[0] {
                *removed_by_task.entry(task.clone()).or_insert(0) += end - begin;
            }
        }

        let no_hint = token.properties.contains(&TokenProperty::NoHint);
        let unimpl = token.properties.contains(&TokenProperty::Unimplemented);
        let stub = token
            .properties
            .iter()
            .filter_map(|property| {
                if let TokenProperty::Stub(stub) = property {
                    Some(stub)
                } else {
                    None
                }
            })
            .nth(0);
        if no_hint {
            if begin > 0 &&
                lines[begin - 1].trim().is_empty() &&
                end < lines.len() &&
                lines[end].trim().is_empty()
            {
                next_pos = end + 1;
            } else {
                next_pos = end;
            }
        } else {
            let mut insert_line = |line: &str| {
                for c in lines[begin].chars() {
                    if c.is_whitespace() {
                        dst.push(c);
                    } else {
                        break;
                    }
                }
                dst.push_str(line);
            };

            if let Some(stub) = stub {
                insert_line(stub);
                dst.push_str(" // TODO: remove before flight.\n");
            } else {
                insert_line("// TODO: your code here.\n");
                if unimpl {
                    insert_line("unimplemented!();\n");
                }
            }

            next_pos = end;
        }
    }

    for line in &lines[next_pos..] {
        dst += line;
        dst += "\n";
    }

    Ok(ProcessedSource {
        src: dst,
        removed_lines,
        removed_by_task,
    })
}

struct Compose {
    args: Opts,

    removed_lines: usize,
    removed_by_task: HashMap<String, usize>,
}

impl Compose {
    fn new(args: &Opts) -> Compose {
        Compose {
            args: args.clone(),

            removed_lines: 0,
            removed_by_task: HashMap::new(),
        }
    }

    fn process_file(&mut self, in_path: &Path, out_path: Option<&Path>) -> Result<()> {
        if in_path.to_str().map(|s| s.ends_with(".rs") || s.ends_with(".s")).unwrap_or(false) {
            let content = fs::read_to_string(in_path)
                .with_context(|| format!("failed to read file {}", in_path.display()))?;

            let new_content = process_source(in_path, content)
                .with_context(|| format!("failed to process file {}", in_path.display()))?;

            self.removed_lines += new_content.removed_lines;

            for (k, v) in new_content.removed_by_task {
                *self.removed_by_task.entry(k).or_insert(0) += v;
            }

            if let Some(out_file) = out_path {
                fs::write(out_file, new_content.src)
                    .with_context(|| format!("failed to write file {}", out_file.display()))?;
            }
        } else if let Some(out_file) = out_path {
            if in_path != out_file {
                fs::copy(in_path, out_file).with_context(|| {
                    format!(
                        "failed to copy {} to {}",
                        in_path.display(),
                        out_file.display()
                    )
                })?;
            }
        }

        Ok(())
    }

    fn process_dir(&mut self, in_path: &Path, out_path: Option<&Path>) -> Result<()> {
        let dir = fs::read_dir(in_path)
            .with_context(|| format!("failed to read dir {}", in_path.display()))?;

        if let Some(out_dir) = out_path {
            fs::create_dir_all(out_dir)
                .with_context(|| format!("failed to create dir {}", out_dir.display()))?;
        }

        for mb_entry in dir {
            let name = mb_entry
                .with_context(|| format!("failed to read entry in dir {}", in_path.display()))?
                .file_name();

            let new_in_path = in_path.join(&name);
            let new_out_path = out_path.map(|p| p.join(&name));

            if self.args.spare.contains(&new_in_path) {
                continue;
            }

            if new_in_path.is_dir() {
                self.process_dir(&new_in_path, new_out_path.as_deref())?;
            } else {
                self.process_file(&new_in_path, new_out_path.as_deref())?;
            }
        }

        Ok(())
    }

    fn process(&mut self) -> Result<()> {
        self.process_dir(
            self.args.in_path.clone().as_path(),
            self.args.out_path.clone().as_deref(),
        )?;

        if self.args.stat {
            for (k, v) in self.removed_by_task.iter().sorted_by_key(|x| x.0) {
                println!("{}\t{}", v, k);
            }

            println!("{}\ttotal", self.removed_lines);
        }

        Ok(())
    }
}


fn do_main(args: Opts) -> Result<()> {
    let mut compose = Compose::new(&args);

    compose.process().context("failed to process entries")?;

    Ok(())
}

fn main() {
    let args = Opts::from_args();

    if let Err(err) = do_main(args) {
        eprintln!("Error: {:#}", err);
        std::process::exit(1);
    }
}
