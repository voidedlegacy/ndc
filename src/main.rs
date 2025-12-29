use std::env;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

const WHITESPACE: &[u8] = b" \r\n";
const DELIMITERS: &[u8] = b" \r\n,():";

type Integer = i64;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorType {
    None = 0,
    Arguments,
    Type,
    Generic,
    Syntax,
    Todo,
    Max,
}

struct Error {
    kind: ErrorType,
    msg: Option<String>,
}

impl Error {
    fn none() -> Self {
        Self {
            kind: ErrorType::None,
            msg: None,
        }
    }
}

fn print_error(err: &Error) {
    if err.kind == ErrorType::None {
        return;
    }
    print!("ERROR: ");
    debug_assert_eq!(ErrorType::Max as u8, 6);
    match err.kind {
        ErrorType::Todo => print!("TODO (not implemented)"),
        ErrorType::Syntax => print!("Invalid syntax"),
        ErrorType::Type => print!("Mismatched types"),
        ErrorType::Arguments => print!("Invalid arguments"),
        ErrorType::Generic => {}
        ErrorType::None => {}
        ErrorType::Max => print!("Unkown error type..."),
    }
    println!();
    if let Some(msg) = &err.msg {
        println!("     : {}", msg);
    }
}

fn file_size(file: &mut File) -> io::Result<u64> {
    let original = file.stream_position()?;
    let size = file.seek(SeekFrom::End(0))?;
    if let Err(e) = file.seek(SeekFrom::Start(original)) {
        println!("fsetpos() failed: {}", e);
    }
    Ok(size)
}

fn file_contents(path: &str) -> Option<Vec<u8>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            println!("Could not open file at {}", path);
            return None;
        }
    };

    if let Err(e) = file_size(&mut file) {
        println!("fgetpos() failed: {}", e);
        return None;
    }

    let mut contents = Vec::new();
    if let Err(e) = file.read_to_end(&mut contents) {
        println!("Error while reading: {}", e);
        return None;
    }
    Some(contents)
}

fn is_in(set: &[u8], b: u8) -> bool {
    set.contains(&b)
}

/// Lex the next token from SOURCE, starting at START.
/// Returns Ok(None) on end of input.
fn lex(source: &[u8], start: usize) -> Result<Option<(usize, usize)>, Error> {
    if start > source.len() {
        return Err(Error {
            kind: ErrorType::Arguments,
            msg: Some("Can not lex empty source.".to_string()),
        });
    }

    let mut beg = start;
    while beg < source.len() && is_in(WHITESPACE, source[beg]) {
        beg += 1;
    }
    if beg >= source.len() {
        return Ok(None);
    }

    let mut end = beg;
    while end < source.len() && !is_in(DELIMITERS, source[end]) {
        end += 1;
    }
    if end == beg {
        end = beg + 1;
    }

    Ok(Some((beg, end)))
}


// TODO:
// 1. API to create new node.
// 2. API to add node as child.
#[allow(dead_code)]
#[derive(Debug)]
enum NodeType {
    None,
    Integer,
    Program,
    Max,
}

#[allow(dead_code)]
#[derive(Debug)]
enum NodeValue {
    None,
    Integer(Integer),
}

#[allow(dead_code)]
#[derive(Debug)]
struct Node {
    kind: NodeType,
    value: NodeValue,
    children: Vec<Box<Node>>,
}

impl Node {
    fn none() -> Self {
        Self {
            kind: NodeType::None,
            value: NodeValue::None,
            children: Vec::new(),
        }
    }
}

#[allow(dead_code)]
fn nonep(node: &Node) -> bool {
    matches!(node.kind, NodeType::None)
}

#[allow(dead_code)]
fn integerp(node: &Node) -> bool {
    matches!(node.kind, NodeType::Integer)
}

// TODO:
// 1. API to create new Binding
// 2. API to add Binding to new environment
#[allow(dead_code)]
#[derive(Debug)]
struct Binding {
    id: String,
    value: Box<Node>,
    next: Option<Box<Binding>>,
}

// TOOD: API to create new environment
#[allow(dead_code)]
#[derive(Debug)]
struct Environment {
    parent: Option<Box<Environment>>,
    bind: Option<Box<Binding>>,
}

#[allow(dead_code)]
fn environment_set() {}

fn parse_expr(source: &[u8], _result: &mut Node) -> Error {
    let mut pos = 0;
    loop {
        match lex(source, pos) {
            Ok(Some((beg, end))) => {
                println!("lexed: {}", String::from_utf8_lossy(&source[beg..end]));
                pos = end;
            }
            Ok(None) => break,
            Err(err) => return err,
        }
    }
    Error::none()
}

fn print_usage(argv0: &str) {
    println!("USAGE: {} <path_to_file_to_compile>", argv0);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    let path = &args[1];
    if let Some(contents) = file_contents(path) {
        let mut expression = Node::none();
        let err = parse_expr(&contents, &mut expression);
        print_error(&err);
    }
}
