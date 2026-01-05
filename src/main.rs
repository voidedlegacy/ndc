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
    assert!(
        !contents.is_empty() || size == 0,
        "Could not allocate buffer for file contents"
    );
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
        _ => print!("Unkown error type..."),
    }
    println!();
    if let Some(msg) = &err.msg {
        println!("     : {}", msg);
    }
}

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

struct Token {
    beginning: usize,
    end: usize,
}

fn print_token(source: &[u8], t: &Token) {
    if t.beginning >= t.end || t.end > source.len() {
        return;
    }
    print!("{}", String::from_utf8_lossy(&source[t.beginning..t.end]));
}

/// Lex the next token from SOURCE, and point to it with BEG and END.
fn lex(source: &[u8], start: usize, token: &mut Token) -> Error {
    let mut err = ok();
    if start > source.len() {
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

// A : integer = 420
//
// PROGRAM
// `-- VARIABLE_DECLARATION_INITIALIZED
//     `-- INTEGER (420) -> SYMBOL (A)

// TODO:
// |-- API to create new node.
// `-- API to add node as child.
#[allow(non_camel_case_types)]
type integer_t = i64;

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeType {
    // BEGIN LITERALS

    /// The definition of nothing; false, etc.
    NODE_TYPE_NONE,

    /// Just an integer.
    NODE_TYPE_INTEGER,

    /// When a literal is expected but no other literal is valid, it
    /// becomes a symbol.
    NODE_TYPE_SYMBOL,

    // END LITERALS

    /// Contains two children. The first determines type (and value),
    /// while the second contains the symbolic name of the variable.
    NODE_TYPE_VARIABLE_DECLARATION,
    NODE_TYPE_VARIABLE_DECLARATION_INITIALIZED,

    /// Contains two children that determine left and right acceptable
    /// types.
    NODE_TYPE_BINARY_OPERATOR,

    /// Contains a list of expressions to execute in sequence.
    NODE_TYPE_PROGRAM,

    NODE_TYPE_MAX,
}

#[derive(Clone, Debug)]
struct NodeValue {
    integer: integer_t,
    symbol: Option<String>,
}

#[derive(Clone, Debug)]
struct Node {
    // TODO: Think about how to document node types and how they fit in the AST.
    type_: NodeType,
    value: NodeValue,
    // Possible TODO: Parent?
    children: Option<Box<Node>>,
    next_child: Option<Box<Node>>,
}

impl Node {
    fn zeroed() -> Self {
        Node {
            type_: NodeType::NODE_TYPE_NONE,
            value: NodeValue {
                integer: 0,
                symbol: None,
            },
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

fn symbolp(node: &Node) -> bool {
    node.type_ == NodeType::NODE_TYPE_SYMBOL
}

fn node_add_child(parent: &mut Node, new_child: &Node) {
    let allocated_child = Box::new(new_child.clone());
    let mut cursor = &mut parent.children;
    loop {
        match cursor {
            Some(child) => {
                cursor = &mut child.next_child;
            }
            None => {
                *cursor = Some(allocated_child);
                break;
            }
        }
    }
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
        _ => 0,
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
        NodeType::NODE_TYPE_SYMBOL => {
            print!("SYM");
            if let Some(symbol) = &node.value.symbol {
                print!(":{}", symbol);
            }
        }
        NodeType::NODE_TYPE_BINARY_OPERATOR => print!("BINARY OPERATOR"),
        NodeType::NODE_TYPE_VARIABLE_DECLARATION => print!("VARIABLE DECLARATION"),
        NodeType::NODE_TYPE_VARIABLE_DECLARATION_INITIALIZED => {
            print!("VARIABLE DECLARATION INITIALIZED");
        }
        NodeType::NODE_TYPE_PROGRAM => print!("PROGRAM"),
        _ => print!("UNKNOWN"),
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
    while let Some(mut child_node) = child {
        let next_child = child_node.next_child.take();
        node_free(Some(child_node));
        child = next_child;
    }
    if symbolp(&root) {
        root.value.symbol = None;
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
    Node::zeroed()
}

// @return Boolean-like value; 1 for success, 0 for failure.
fn token_string_equalp(string: &str, token: &Token, source: &[u8]) -> i32 {
    if string.is_empty() {
        return 1;
    }
    let bytes = string.as_bytes();
    let mut i = 0usize;
    let mut beg = token.beginning;
    while i < bytes.len() && beg < token.end {
        if beg >= source.len() {
            return 0;
        }
        if bytes[i] != source[beg] {
            return 0;
        }
        i += 1;
        beg += 1;
    }
    1
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
    } else if let Ok(token_str) = std::str::from_utf8(token_slice) {
        if let Ok(value) = token_str.parse::<i64>() {
            if value == 0 {
                return 0;
            }
            node.type_ = NodeType::NODE_TYPE_INTEGER;
            node.value.integer = value;
        } else {
            return 0;
        }
    } else {
        return 0;
    }
    1
}

fn parse_expr(source: &[u8], end: &mut usize, result: &mut Node) -> Error {
    let _token_count: usize = 0;
    let mut current_token = Token {
        beginning: 0,
        end: 0,
    };
    let mut err = ok();

    while {
        err = lex(source, current_token.end, &mut current_token);
        err.type_ == ErrorType::ERROR_NONE
    } {
        *end = current_token.end;
        let token_length = current_token.end.saturating_sub(current_token.beginning);
        if token_length == 0 {
            break;
        }
        if parse_integer(source, &current_token, result) != 0 {
            // look ahead for binary ops that include integers.
            let _lhs_integer = (*result).clone();
            err = lex(source, current_token.end, &mut current_token);
            if err.type_ != ErrorType::ERROR_NONE {
                return err;
            }
            *end = current_token.end;

            // TODO: Check for valid integer operator.
            // It would be cool to use an operator environment to look up
            // operators instead of hard-coding them. This would eventually
            // allow for user-defined operators, or stuff like that!

        } else {
            // TODO: Check for unary prefix operators.

            // TODO: Check that it isn't a binary operator (we should encounter left
            // side first and peek forward, rather than encounter it at top level).

            let mut symbol = Node::zeroed();
            symbol.type_ = NodeType::NODE_TYPE_SYMBOL;
            symbol.children = None;
            symbol.next_child = None;
            symbol.value.symbol = None;

            let symbol_string =
                String::from_utf8_lossy(&source[current_token.beginning..current_token.end])
                    .to_string();
            symbol.value.symbol = Some(symbol_string);

            *result = symbol.clone();

            // TODO: Check if valid symbol for variable environment, then
            // attempt to pattern match variable access, assignment,
            // declaration, or declaration with initialization.

            err = lex(source, current_token.end, &mut current_token);
            if err.type_ != ErrorType::ERROR_NONE {
                return err;
            }
            *end = current_token.end;
            let token_length = current_token.end.saturating_sub(current_token.beginning);
            if token_length == 0 {
                break;
            }

            if token_string_equalp(":", &current_token, source) != 0 {
                err = lex(source, current_token.end, &mut current_token);
                if err.type_ != ErrorType::ERROR_NONE {
                    return err;
                }
                *end = current_token.end;
                let token_length = current_token.end.saturating_sub(current_token.beginning);
                if token_length == 0 {
                    break;
                }

                // TODO: Look up type in types environment from parsing context.
                if token_string_equalp("integer", &current_token, source) != 0 {
                    let mut var_decl = Node::zeroed();
                    var_decl.children = None;
                    var_decl.next_child = None;
                    var_decl.type_ = NodeType::NODE_TYPE_VARIABLE_DECLARATION;

                    let mut type_node = Node::zeroed();
                    type_node.type_ = NodeType::NODE_TYPE_INTEGER;

                    node_add_child(&mut var_decl, &type_node);
                    node_add_child(&mut var_decl, &symbol);

                    *result = var_decl;

                    // TODO: Look ahead for "=" assignment operator.

                    return ok();
                }
            }

            print!("Unrecognized token: ");
            print_token(source, &current_token);
            println!();

            return err;
        }

        print!("Intermediate node: ");
        print_node(Some(&*result), 0);
        println!();
    }

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

        // TODO: Create API to heap allocate a program node, as well as add
        // expressions as children.
        let mut expression = Node::zeroed();
        let mut contents_it = 0usize;
        let err = parse_expr(&contents, &mut contents_it, &mut expression);
        print_node(Some(&expression), 0);
        println!();

        print_error(&err);
    }
}
