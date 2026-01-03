use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn file_size(file: &mut File) -> usize {
    let original = match file.stream_position() {
        Ok(pos) => pos,
        Err(e) => {
            println!("fgetpos() failed: {}", e.raw_os_error().unwrap_or(0));
            return 0;
        }
    };
    let out = match file.seek(SeekFrom::End(0)) {
        Ok(pos) => pos,
        Err(_) => return 0,
    };
    if let Err(e) = file.seek(SeekFrom::Start(original)) {
        println!("fsetpos() failed: {}", e.raw_os_error().unwrap_or(0));
    }
    out as usize
}

fn file_contents(path: &str) -> Option<Vec<u8>> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            println!("Could not open file at {}", path);
            return None;
        }
    };
    let size = file_size(&mut file);
    let mut contents = vec![0u8; size + 1];
    let mut bytes_read = 0usize;
    while bytes_read < size {
        let bytes_read_this_iteration = match file.read(&mut contents[bytes_read..size]) {
            Ok(n) => n,
            Err(e) => {
                println!("Error while reading: {}", e.raw_os_error().unwrap_or(0));
                return None;
            }
        };
        bytes_read += bytes_read_this_iteration;
        if bytes_read_this_iteration == 0 {
            break;
        }
    }
    contents[bytes_read] = 0;
    Some(contents)
}

fn print_usage(argv0: &str) {
    println!("USAGE: {} <path_to_file_to_compile>", argv0);
}

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ErrorType {
    ERROR_NONE = 0,
    ERROR_ARGUMENTS,
    ERROR_TYPE,
    ERROR_GENERIC,
    ERROR_SYNTAX,
    ERROR_TODO,
    ERROR_MAX,
}

#[derive(Clone, Debug)]
struct Error {
    type_: ErrorType,
    msg: Option<String>,
}

fn ok() -> Error {
    Error {
        type_: ErrorType::ERROR_NONE,
        msg: None,
    }
}

fn print_error(err: &Error) {
    if err.type_ == ErrorType::ERROR_NONE {
        return;
    }
    print!("ERROR: ");
    debug_assert_eq!(ErrorType::ERROR_MAX as i32, 6);
    match err.type_ {
        ErrorType::ERROR_TODO => print!("TODO (not implemented)"),
        ErrorType::ERROR_SYNTAX => print!("Invalid syntax"),
        ErrorType::ERROR_TYPE => print!("Mismatched types"),
        ErrorType::ERROR_ARGUMENTS => print!("Invalid arguments"),
        ErrorType::ERROR_GENERIC => {}
        ErrorType::ERROR_NONE => {}
        ErrorType::ERROR_MAX => print!("Unkown error type..."),
    }
    println!();
    if let Some(msg) = &err.msg {
        println!("     : {}", msg);
    }
}

#[allow(dead_code)]
fn error_create(kind: ErrorType, msg: &str) -> Error {
    Error {
        type_: kind,
        msg: Some(msg.to_string()),
    }
}

fn error_prep(err: &mut Error, kind: ErrorType, message: &str) {
    err.type_ = kind;
    err.msg = Some(message.to_string());
}

const WHITESPACE: &[u8] = b" \r\n";
const DELIMITERS: &[u8] = b" \r\n,():";

#[derive(Clone, Debug)]
struct Token {
    beginning: usize,
    end: usize,
    next: Option<Box<Token>>,
}

fn token_create() -> Box<Token> {
    Box::new(Token {
        beginning: 0,
        end: 0,
        next: None,
    })
}

fn token_free(mut root: Option<Box<Token>>) {
    while let Some(mut token_to_free) = root {
        root = token_to_free.next.take();
    }
}

fn print_token(source: &[u8], t: &Token) {
    if t.beginning >= t.end || t.end > source.len() {
        return;
    }
    print!("{}", String::from_utf8_lossy(&source[t.beginning..t.end]));
}

fn print_tokens(source: &[u8], mut root: Option<&Token>) {
    let mut count: usize = 1;
    while let Some(token) = root {
        // FIXME: Remove this limit.
        if count > 10000 {
            break;
        }
        print!("Token {}: ", count);
        if token.beginning < token.end && token.end <= source.len() {
            print!(
                "{}",
                String::from_utf8_lossy(&source[token.beginning..token.end])
            );
        }
        println!();
        root = token.next.as_deref();
        count += 1;
    }
}

/// Lex the next token from SOURCE, and point to it with BEG and END.
fn lex(source: &[u8], start: usize, token: &mut Token) -> Error {
    let mut err = ok();
    if source.is_empty() || start >= source.len() {
        error_prep(&mut err, ErrorType::ERROR_ARGUMENTS, "Can not lex empty source.");
        return err;
    }
    token.beginning = start;
    while token.beginning < source.len() && WHITESPACE.contains(&source[token.beginning]) {
        token.beginning += 1;
    }
    token.end = token.beginning;
    if token.end >= source.len() {
        return err;
    }
    if source[token.end] == 0 {
        return err;
    }
    while token.end < source.len()
        && !DELIMITERS.contains(&source[token.end])
        && source[token.end] != 0
    {
        token.end += 1;
    }
    if token.end == token.beginning {
        token.end += 1;
    }
    err
}

//      Node-
//     /  |  \
//    0   1   2
//   / \
//  3   4
//
// Node
// `-- 0  ->  1  ->  2
//     `-- 3  ->  4

// TODO:
// |-- API to create new node.
// `-- API to add node as child.
#[allow(non_camel_case_types)]
type integer_t = i64;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeType {
    NODE_TYPE_NONE,
    NODE_TYPE_INTEGER,
    NODE_TYPE_PROGRAM,
    NODE_TYPE_MAX,
}

#[derive(Clone, Copy, Debug)]
struct NodeValue {
    integer: integer_t,
}

#[derive(Clone, Debug)]
struct Node {
    type_: NodeType,
    value: NodeValue,
    // Possible TODO: Parent?
    children: Option<Box<Node>>,
    next_child: Option<Box<Node>>,
}

impl Node {
    fn none() -> Self {
        Node {
            type_: NodeType::NODE_TYPE_NONE,
            value: NodeValue { integer: 0 },
            children: None,
            next_child: None,
        }
    }
}

fn nonep(node: &Node) -> bool {
    node.type_ == NodeType::NODE_TYPE_NONE
}

fn integerp(node: &Node) -> bool {
    node.type_ == NodeType::NODE_TYPE_INTEGER
}

/// @return Boolean-like value; 1 for success, 0 for failure.
fn node_compare(a: Option<&Node>, b: Option<&Node>) -> i32 {
    if a.is_none() || b.is_none() {
        if a.is_none() && b.is_none() {
            return 1;
        }
        return 0;
    }
    let a = a.unwrap();
    let b = b.unwrap();
    debug_assert_eq!(NodeType::NODE_TYPE_MAX as i32, 3, "node_compare() must handle all node types");
    if a.type_ != b.type_ {
        return 0;
    }
    match a.type_ {
        NodeType::NODE_TYPE_NONE => {
            if nonep(b) {
                return 1;
            }
            0
        }
        NodeType::NODE_TYPE_INTEGER => {
            if a.value.integer == b.value.integer {
                return 1;
            }
            0
        }
        NodeType::NODE_TYPE_PROGRAM => {
            // TODO: Compare two programs.
            println!("TODO: Compare two programs.");
            0
        }
        NodeType::NODE_TYPE_MAX => 0,
    }
}

fn print_node(node: Option<&Node>, indent_level: usize) {
    if node.is_none() {
        return;
    }
    let node = node.unwrap();

    // Print indent.
    for _ in 0..indent_level {
        print!(" ");
    }
    // Print type + value.
    debug_assert_eq!(NodeType::NODE_TYPE_MAX as i32, 3, "print_node() must handle all node types");
    match node.type_ {
        NodeType::NODE_TYPE_NONE => print!("NONE"),
        NodeType::NODE_TYPE_INTEGER => print!("INT:{}", node.value.integer),
        NodeType::NODE_TYPE_PROGRAM => print!("PROGRAM"),
        NodeType::NODE_TYPE_MAX => {
            print!("UNKNOWN");
            print!("NONE");
        }
    }
    println!();
    // Print children.
    let mut child = node.children.as_deref();
    while let Some(child_node) = child {
        print_node(Some(child_node), indent_level + 4);
        child = child_node.next_child.as_deref();
    }
}

// TODO: Make more efficient! m_n_t_r_a on Twitch suggests keeping track
// of allocated pointers and then freeing all in one go.
fn node_free(root: Option<Box<Node>>) {
    if root.is_none() {
        return;
    }
    let mut root = root.unwrap();
    let mut child = root.children.take();
    let mut next_child: Option<Box<Node>> = None;
    while let Some(mut child_node) = child {
        next_child = child_node.next_child.take();
        node_free(Some(child_node));
        child = next_child;
    }
}

// TODO:
// |-- API to create new Binding.
// `-- API to add Binding to environment.
struct Binding {
    id: Node,
    value: Node,
    next: Option<Box<Binding>>,
}

// TODO: API to create new Environment.
struct Environment {
    parent: Option<Box<Environment>>,
    bind: Option<Box<Binding>>,
}

fn environment_create(parent: Option<Box<Environment>>) -> Box<Environment> {
    Box::new(Environment { parent, bind: None })
}

fn environment_set(env: &mut Environment, id: Node, value: Node) {
    // Over-write existing value if ID is already bound in environment.
    let mut binding_it = env.bind.as_deref_mut();
    while let Some(binding) = binding_it {
        if node_compare(Some(&binding.id), Some(&id)) != 0 {
            binding.value = value;
            return;
        }
        binding_it = binding.next.as_deref_mut();
    }
    // Create new binding.
    let mut binding = Box::new(Binding {
        id,
        value,
        next: None,
    });
    binding.next = env.bind.take();
    env.bind = Some(binding);
}

fn environment_get(env: &Environment, id: Node) -> Node {
    let mut binding_it = env.bind.as_deref();
    while let Some(binding) = binding_it {
        if node_compare(Some(&binding.id), Some(&id)) != 0 {
            return binding.value.clone();
        }
        binding_it = binding.next.as_deref();
    }
    Node::none()
}

// @return Boolean-like value; 1 for success, 0 for failure.
fn token_string_equalp(string: &str, token: &Token, source: &[u8]) -> i32 {
    let bytes = string.as_bytes();
    let mut i = 0usize;
    let mut beg = token.beginning;
    while i < bytes.len() && beg < token.end && beg < source.len() {
        if bytes[i] != source[beg] {
            return 0;
        }
        i += 1;
        beg += 1;
    }
    if i == bytes.len() && beg == token.end {
        return 1;
    }
    0
}

/// @return Boolean-like value; 1 upon success, 0 for failure.
fn parse_integer(source: &[u8], token: &Token, node: &mut Node) -> i32 {
    if token.end <= token.beginning || token.end > source.len() {
        return 0;
    }
    let token_slice = &source[token.beginning..token.end];
    if token_slice.len() == 1 && token_slice[0] == b'0' {
        node.type_ = NodeType::NODE_TYPE_INTEGER;
        node.value.integer = 0;
        return 1;
    }
    let mut idx = 0usize;
    let mut sign: i128 = 1;
    if token_slice[0] == b'+' || token_slice[0] == b'-' {
        if token_slice[0] == b'-' {
            sign = -1;
        }
        idx = 1;
    }
    let start_digits = idx;
    while idx < token_slice.len() && token_slice[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx == start_digits {
        return 0;
    }
    let mut value: i128 = 0;
    for &b in &token_slice[start_digits..idx] {
        value = value * 10 + (b - b'0') as i128;
    }
    value *= sign;
    let value = if value > i64::MAX as i128 {
        i64::MAX
    } else if value < i64::MIN as i128 {
        i64::MIN
    } else {
        value as i64
    };
    if value != 0 {
        node.type_ = NodeType::NODE_TYPE_INTEGER;
        node.value.integer = value;
        return 1;
    }
    0
}

fn parse_expr(source: &[u8], _result: &mut Node) -> Error {
    let mut token_count: usize = 0;
    let mut current_token = Token {
        beginning: 0,
        end: 0,
        next: None,
    };
    let mut err = ok();

    let _root = Box::new(Node {
        type_: NodeType::NODE_TYPE_PROGRAM,
        value: NodeValue { integer: 0 },
        children: None,
        next_child: None,
    });

    loop {
        err = lex(source, current_token.end, &mut current_token);
        if err.type_ != ErrorType::ERROR_NONE {
            break;
        }
        let mut working_node = Node::none();
        let token_length = current_token.end.saturating_sub(current_token.beginning);
        if token_length == 0 {
            break;
        }
        token_count += 1;
        if parse_integer(source, &current_token, &mut working_node) != 0 {
            // look ahead for binary ops that include integers.
            let _integer = current_token.clone();
            err = lex(source, current_token.end, &mut current_token);
            if err.type_ != ErrorType::ERROR_NONE {
                return err;
            }
            // TODO: Check for valid integer operator.
            // It would be cool to use an operator environment to look up
            // operators instead of hard-coding them. This would eventually
            // allow for user-defined operators, or stuff like that!
        } else {
            print!("Unrecognized token: ");
            print_token(source, &current_token);
            println!();

            // TODO: Check if valid symbol for variable environment, then
            // attempt to pattern match variable access, assignment,
            // declaration, or declaration with initialization.

        }
        print!("Found node: ");
        print_node(Some(&working_node), 0);
        println!();
    }

    let _ = token_count;
    err
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage(&args[0]);
        return;
    }

    let path = &args[1];
    let contents = file_contents(path);
    if let Some(contents) = contents {
        //printf("Contents of %s:\n---\n\"%s\"\n---\n", path, contents);

        let mut expression = Node::none();
        let err = parse_expr(&contents, &mut expression);
        print_error(&err);
    }
}
