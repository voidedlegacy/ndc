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

// TODO: Add file path, byte offset, etc.
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

fn node_allocate() -> Box<Node> {
    Box::new(Node {
        type_: NodeType::NODE_TYPE_NONE,
        value: NodeValue {
            integer: 0,
            symbol: None,
        },
        children: None,
        next_child: None,
    })
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

/// PARENT is modified, NEW_CHILD pointer is used verbatim.
fn node_add_child(parent: &mut Node, new_child: Box<Node>) {
    if parent.children.is_none() {
        parent.children = Some(new_child);
        return;
    }

    let mut cursor = parent.children.as_mut();
    while let Some(child) = cursor {
        if child.next_child.is_none() {
            child.next_child = Some(new_child);
            return;
        }
        cursor = child.next_child.as_mut();
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
    // TODO: This assert doesn't work, I don't know why :^(.
    debug_assert_eq!(NodeType::NODE_TYPE_MAX as i32, 7, "node_compare() must handle all node types");
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
        NodeType::NODE_TYPE_SYMBOL => {
            match (&a.value.symbol, &b.value.symbol) {
                (Some(left), Some(right)) => {
                    if left == right {
                        1
                    } else {
                        0
                    }
                }
                (None, None) => 1,
                _ => 0,
            }
        }
        NodeType::NODE_TYPE_BINARY_OPERATOR => {
            println!("TODO: node_compare() BINARY OPERATOR");
            0
        }
        NodeType::NODE_TYPE_VARIABLE_DECLARATION => {
            println!("TODO: node_compare() VARIABLE DECLARATION");
            0
        }
        NodeType::NODE_TYPE_VARIABLE_DECLARATION_INITIALIZED => {
            println!("TODO: node_compare() VARIABLE DECLARATION INITIALIZED");
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

fn node_integer(value: i64) -> Box<Node> {
    let mut integer = node_allocate();
    integer.type_ = NodeType::NODE_TYPE_INTEGER;
    integer.value.integer = value;
    integer.children = None;
    integer.next_child = None;
    integer
}

// TODO: Think about caching used symbols and not creating duplicates!
fn node_symbol(symbol_string: &str) -> Box<Node> {
    let mut symbol = node_allocate();
    symbol.type_ = NodeType::NODE_TYPE_SYMBOL;
    symbol.value.symbol = Some(symbol_string.to_string());
    symbol
}

fn node_symbol_from_buffer(buffer: &[u8]) -> Box<Node> {
    assert!(!buffer.is_empty(), "Can not create AST symbol node from NULL buffer");
    let symbol_string = String::from_utf8_lossy(buffer).to_string();
    let mut symbol = node_allocate();
    symbol.type_ = NodeType::NODE_TYPE_SYMBOL;
    symbol.value.symbol = Some(symbol_string);
    symbol
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
    debug_assert_eq!(NodeType::NODE_TYPE_MAX as i32, 7, "print_node() must handle all node types");
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
    id: Box<Node>,
    value: Box<Node>,
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

/**
 * @retval 0 Failure.
 * @retval 1 Creation of new binding.
 * @retval 2 Existing binding value overwrite (ID unused).
 */
fn environment_set(env: &mut Environment, id: Box<Node>, value: Box<Node>) -> i32 {
    // Over-write existing value if ID is already bound in environment.
    if id.type_ == NodeType::NODE_TYPE_NONE && value.type_ == NodeType::NODE_TYPE_NONE {
        return 0;
    }
    let mut binding_it = env.bind.as_deref_mut();
    while let Some(binding) = binding_it {
        if node_compare(Some(&binding.id), Some(&id)) != 0 {
            binding.value = value;
            return 2;
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
    1
}

/// @return Boolean-like value; 1 for success, 0 for failure.
fn environment_get(env: &Environment, id: &Node, result: &mut Node) -> i32 {
    let mut binding_it = env.bind.as_deref();
    while let Some(binding) = binding_it {
        if node_compare(Some(&binding.id), Some(id)) != 0 {
            *result = (*binding.value).clone();
            return 1;
        }
        binding_it = binding.next.as_deref();
    }
    0
}

fn environment_get_by_symbol(env: &Environment, symbol: &str, result: &mut Node) -> i32 {
    let symbol_node = node_symbol(symbol);
    let status = environment_get(env, &symbol_node, result);
    status
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

type ParsingContext = ParsingContextStruct;

struct ParsingContextStruct {
    // FIXME: "struct ParsingContext *parent;" ???
    types: Box<Environment>,
    variables: Box<Environment>,
}

fn parse_context_create() -> Box<ParsingContextStruct> {
    let mut ctx = Box::new(ParsingContextStruct {
        types: environment_create(None),
        variables: environment_create(None),
    });
    if environment_set(&mut ctx.types, node_symbol("integer"), node_integer(0)) == 0 {
        println!("ERROR: Failed to set builtin type in types environment.");
    }
    ctx
}

fn parse_expr(
    context: &mut ParsingContext,
    source: &[u8],
    end: &mut usize,
    result: &mut Node,
) -> Error {
    let _token_count: usize = 0;
    let mut current_token = Token {
        beginning: 0,
        end: 0,
    };
    let mut err;

    loop {
        err = lex(source, current_token.end, &mut current_token);
        if err.type_ != ErrorType::ERROR_NONE {
            break;
        }
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

            let symbol = node_symbol_from_buffer(
                &source[current_token.beginning..current_token.beginning + token_length],
            );

            //*result = *symbol;

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

                let expected_type_symbol = node_symbol_from_buffer(
                    &source[current_token.beginning..current_token.beginning + token_length],
                );
                let status = environment_get(&context.types, &expected_type_symbol, result);
                if status == 0 {
                    error_prep(
                        &mut err,
                        ErrorType::ERROR_TYPE,
                        "Invalid type within variable declaration",
                    );
                    println!(
                        "\nINVALID TYPE: \"{}\"",
                        expected_type_symbol.value.symbol.as_deref().unwrap_or("")
                    );
                    return err;
                } else {
                    //printf("Found valid type: ");
                    //print_node(expected_type_symbol,0);
                    //putchar('\n');

                    let mut var_decl = node_allocate();
                    var_decl.type_ = NodeType::NODE_TYPE_VARIABLE_DECLARATION;

                    let mut type_node = node_allocate();
                    type_node.type_ = result.type_;

                    node_add_child(&mut var_decl, type_node);
                    node_add_child(&mut var_decl, symbol);

                    *result = (*var_decl).clone();

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
        let mut context = parse_context_create();
        let mut program = node_allocate();
        program.type_ = NodeType::NODE_TYPE_PROGRAM;
        let mut expression = node_allocate();
        *expression = Node {
            type_: NodeType::NODE_TYPE_NONE,
            value: NodeValue {
                integer: 0,
                symbol: None,
            },
            children: None,
            next_child: None,
        };
        let mut contents_it = 0usize;
        let err = parse_expr(&mut context, &contents, &mut contents_it, &mut expression);
        node_add_child(&mut program, expression);

        print_error(&err);

        print_node(Some(&program), 0);
        println!();

        node_free(Some(program));
    }
}
