# KAIN Language Reference for LLMs

> Optimized context for AI code generation. Minimal tokens, maximum understanding.

## Syntax Rules (Exhaustive)

```
INDENTATION: 4 spaces = block scope (Python-style)
COMMENTS:    // single line, /* multi-line */
STRINGS:     "double quotes", """raw multi-line"""
TYPES:       Int, Float, Bool, String, Array<T>, Map<K,V>, Option<T>, Result<T,E>
```

## Type Declarations

```kain
struct Name:
    field: Type
    field2: Type = default_value

enum Name:
    Variant1
    Variant2(Type)
    Variant3(Type, Type)

trait Name:
    fn method(self) -> ReturnType

impl StructName:
    pub fn method(self, param: Type) -> ReturnType:
        body

impl TraitName for StructName:
    fn method(self) -> ReturnType:
        body
```

## Functions

```kain
fn name(param: Type, param2: Type) -> ReturnType:
    body
    return value

fn name(param: Type) -> ReturnType with Effect:
    body  // Effect = Pure, IO, Async, GPU, Unsafe

pub fn public_function() -> Type:
    body

async fn async_function() -> Future<Type>:
    let result = await other_async()
    return result
```

## Variables

```KAIN
let immutable: Type = value     // Immutable binding
var mutable: Type = value       // Mutable binding
let inferred = value            // Type inference
```

## Control Flow

```kain
if condition:
    body
else if other:
    body
else:
    body

while condition:
    body

for item in iterable:
    body

for i in range(0, 10):
    body

loop:
    if done: break
    continue

match value:
    Pattern1 => result1
    Pattern2(x) => use(x)
    _ => default
```

## Pattern Matching

```kain
match option:
    Some(x) => x
    None => default

match result:
    Ok(value) => value
    Err(e) => panic(e)

match tuple:
    (a, b, c) => a + b + c

match enum_val:
    EnumName::Variant1 => handle1()
    EnumName::Variant2(data) => handle2(data)
```

## Expressions

```kain
// Binary operators
a + b, a - b, a * b, a / b, a % b
a == b, a != b, a < b, a > b, a <= b, a >= b
a && b, a || b, !a

// Ternary
let x = if cond: val1 else: val2

// Method chaining
object.method().other_method()

// Field access
struct_instance.field

// Index access  
array[index]
map[key]
```

## Collections

```kain
// Arrays
let arr: Array<Int> = [1, 2, 3]
push(arr, 4)
let item = arr[0]
let length = len(arr)

// Maps
let map: Map<String, Int> = Map::new()
map["key"] = value
let val = map["key"]
if contains_key(map, "key"): use(map["key"])

// Tuples
let tuple = (1, "hello", true)
let (a, b, c) = tuple
```

## Option and Result

```kain
// Option
fn find(id: Int) -> Option<User>:
    if exists: return Some(user)
    return None

match find(1):
    Some(user) => use(user)
    None => handle_missing()

// Result
fn parse(s: String) -> Result<Int, String>:
    if valid: return Ok(value)
    return Err("parse error")

let value = parse("42")?  // Propagate errors with ?
```

## Actors (Concurrency)

```kain
actor Counter:
    var count: Int = 0
    
    on Increment(n: Int):
        count = count + n
    
    on GetCount -> Int:
        return count

// Usage
let counter = spawn(Counter)
send(counter, Increment(5))
let n = await send(counter, GetCount)
```

## JSX (UI Components)

```kain
component Button:
    props:
        label: String
        onClick: fn() -> Unit
    
    render:
        <button class="btn" onClick={props.onClick}>
            {props.label}
        </button>

// Usage in render
<div>
    <Button label="Click me" onClick={handleClick} />
    <ul>
        {for item in items:
            <li key={item.id}>{item.name}</li>
        }
    </ul>
</div>
```

## Effects

```kain
fn pure_function(x: Int) -> Int with Pure:
    return x * 2  // No side effects allowed

fn io_function() -> String with IO:
    return read_file("data.txt")  // IO allowed

fn combined() -> Result<Data, Error> with IO, Async:
    let response = await http_get(url)?
    return parse(response)
```

## Standard Library Functions

```
// I/O
println(value)                    // Print with newline
print(value)                      // Print without newline
read_file(path) -> String         // Read file contents
write_file(path, content) -> Bool // Write to file
file_exists(path) -> Bool         // Check file exists

// Strings
len(s) -> Int                     // String length
substring(s, start, end) -> String
starts_with(s, prefix) -> Bool
contains(s, substr) -> Bool
replace(s, old, new) -> String
split(s, delim) -> Array<String>
join(arr, sep) -> String
str(value) -> String              // Convert to string

// Arrays
len(arr) -> Int
push(arr, item)
pop(arr) -> T
contains(arr, item) -> Bool
range(start, end) -> Array<Int>

// Type conversion
int(value) -> Int
float(value) -> Float
str(value) -> String

// Enums
variant_of(enum_val) -> String    // Get variant name
variant_field(enum_val, idx) -> T // Get variant payload

// Control
panic(msg)                        // Abort with message
assert(cond)                      // Assert condition
```

## Complete Example

```kain
use std::io
use std::json

struct User:
    id: Int
    name: String
    email: String

enum ApiResult:
    Success(User)
    NotFound
    Error(String)

impl User:
    pub fn from_json(data: Map<String, Value>) -> Option<User>:
        let id = data["id"]?
        let name = data["name"]?
        let email = data["email"]?
        return Some(User { id: int(id), name: str(name), email: str(email) })

fn fetch_user(id: Int) -> ApiResult with IO, Async:
    let response = await http_get("/api/users/" + str(id))
    
    match response.status:
        200 =>
            match User::from_json(json_parse(response.body)):
                Some(user) => ApiResult::Success(user)
                None => ApiResult::Error("Invalid JSON")
        404 => ApiResult::NotFound
        _ => ApiResult::Error("HTTP " + str(response.status))

fn main() with IO:
    match fetch_user(42):
        ApiResult::Success(user) =>
            println("Found: " + user.name)
        ApiResult::NotFound =>
            println("User not found")
        ApiResult::Error(msg) =>
            println("Error: " + msg)
```

## Key Differences from Similar Languages

| Kain | Rust | Python | TypeScript |
|------|------|--------|------------|
| `fn name():` | `fn name() {` | `def name():` | `function name() {` |
| `let x = 1` | `let x = 1;` | `x = 1` | `const x = 1;` |
| `var x = 1` | `let mut x = 1;` | `x = 1` | `let x = 1;` |
| `Array<T>` | `Vec<T>` | `list[T]` | `T[]` |
| `Option<T>` | `Option<T>` | `Optional[T]` | `T \| null` |
| `with IO` | N/A | N/A | N/A |
| 4-space indent | braces | 4-space | braces |

## Common Patterns

```kain
// Error propagation
fn process() -> Result<Data, Error>:
    let file = read_file(path)?      // Early return on Err
    let parsed = parse(file)?
    return Ok(transform(parsed))

// Builder pattern
let config = Config::new()
    .with_timeout(30)
    .with_retries(3)
    .build()

// Iteration with index
for i, item in enumerate(items):
    println(str(i) + ": " + item)

// Map/filter/fold
let doubled = map(numbers, fn(x) => x * 2)
let evens = filter(numbers, fn(x) => x % 2 == 0)
let sum = fold(numbers, 0, fn(acc, x) => acc + x)
```

---

*Generated for LLM consumption. Revision: 2026-01-31*
