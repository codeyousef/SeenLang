# Collections

## Array\<T\>

The built-in dynamic array type. All Seen programs have access to `Array<T>`.

### Construction

```seen
let arr = Array<Int>()                // empty
let arr = [1, 2, 3, 4, 5]            // literal
let arr = Array<Int>.withLength(100)  // pre-sized
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `push(value: T)` | `Void` | Append element |
| `pop()` | `T` | Remove and return last element |
| `get(index: Int)` | `T` | Get element at index |
| `set(index: Int, value: T)` | `Void` | Set element at index |
| `length()` | `Int` | Number of elements |

### Runtime Array Functions

| Function | Description |
|----------|-------------|
| `seen_arr_new_str()` | Create empty string array |
| `seen_arr_new_ptr()` | Create empty pointer array |
| `seen_arr_push_str(arr, s)` | Push string |
| `seen_arr_push_i64(arr, val)` | Push integer |
| `seen_arr_push_f64(arr, val)` | Push float |
| `seen_arr_push_ptr(arr, ptr)` | Push pointer |
| `seen_arr_get_str(arr, idx)` | Get string at index |
| `seen_arr_get_i64(arr, idx)` | Get integer at index |
| `seen_arr_set_i64(arr, idx, val)` | Set integer at index |
| `seen_arr_set_str(arr, idx, s)` | Set string at index |
| `seen_arr_length(arr)` | Get array length |

### Example

```seen
var names = Array<String>()
names.push("Alice")
names.push("Bob")
names.push("Charlie")

for name in names {
    println("Hello, {name}!")
}
```

## Vec\<T\>

Chunked vector with amortized O(1) push/pop. Better for very large collections.

```seen
import collections.vec
```

### Construction

```seen
let v = Vec<Int>.new()
let v = Vec<Int>.withCapacity(1000)
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `push(value: T)` | `Void` | Append element |
| `pop()` | `T` | Remove last |
| `get(index: Int)` | `T` | Get by index |
| `set(index: Int, value: T)` | `Void` | Set by index |
| `len()` | `Int` | Length |
| `isEmpty()` | `Bool` | Check empty |
| `capacity()` | `Int` | Total capacity |
| `clear()` | `Void` | Remove all elements |
| `reverse()` | `Void` | Reverse in place |

## HashMap\<K, V\>

Robin-hood hashing hash map. Iteration order is non-deterministic.

```seen
import collections.hash_map
```

### Construction

```seen
let map = HashMap<String, Int>()
let map = HashMap<String, Int>.withCapacity(100)
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `insert(key: K, value: V)` | `Void` | Insert or update |
| `get(key: K)` | `Option<V>` | Lookup by key |
| `remove(key: K)` | `Option<V>` | Remove by key |
| `len()` | `Int` | Number of entries |
| `isEmpty()` | `Bool` | Check empty |
| `clear()` | `Void` | Remove all entries |

### Runtime HashMap Functions

4 variants per operation (key types: `int_int`, `int_str`, `str_int`, `str_str`):

| Function | Description |
|----------|-------------|
| `hashmap_new_*()` | Create new map |
| `hashmap_new_*_with_capacity(cap)` | Create with capacity |
| `hashmap_insert_*(map, key, val)` | Insert |
| `hashmap_get_*(map, key)` | Lookup |
| `hashmap_remove_*(map, key)` | Remove |
| `hashmap_clear_*(map)` | Clear all entries |
| `hashmap_size_*(map)` | Get size |

### Example

```seen
var scores = HashMap<String, Int>()
scores.insert("Alice", 95)
scores.insert("Bob", 87)
scores.insert("Charlie", 92)

let alice = scores.get("Alice")
if alice.isSome() {
    println("Alice's score: {alice.unwrap()}")
}
```

## BTreeMap\<K, V\>

Ordered map based on B-tree. Keys are kept sorted.

```seen
import collections.btree_map
```

Same interface as HashMap, but with ordered iteration.

### Runtime BTreeMap Functions

4 variants (`int_int`, `int_str`, `str_int`, `str_str`):

| Function | Description |
|----------|-------------|
| `btreemap_new_*()` | Create new map |
| `btreemap_insert_*(map, key, val)` | Insert (maintains order) |
| `btreemap_get_*(map, key)` | Lookup |
| `btreemap_remove_*(map, key)` | Remove |
| `btreemap_clear_*(map)` | Clear |
| `btreemap_size_*(map)` | Size |

## HashSet\<T\>

Unordered set backed by `HashMap<T, Unit>`.

```seen
import collections.hashset
```

## BTreeSet\<T\>

Ordered set backed by `BTreeMap<T, Unit>`.

```seen
import collections.btree_set
```

## LinkedList\<T\>

Doubly-linked list.

```seen
import collections.linked_list
```

### Methods

| Method | Return | Description |
|--------|--------|-------------|
| `push_front(value: T)` | `Void` | Add to front |
| `push_back(value: T)` | `Void` | Add to back |
| `pop_front()` | `T` | Remove from front |
| `pop_back()` | `T` | Remove from back |
| `len()` | `Int` | Length |

### Runtime LinkedList Functions

2 variants (`int`, `str`):

| Function | Description |
|----------|-------------|
| `linkedlist_new_*()` | Create new list |
| `linkedlist_push_front_*(list, val)` | Push front |
| `linkedlist_push_back_*(list, val)` | Push back |
| `linkedlist_pop_front_*(list)` | Pop front |
| `linkedlist_pop_back_*(list)` | Pop back |
| `linkedlist_size_*(list)` | Size |

## VecDeque\<T\>

Double-ended queue with O(1) front/back operations.

```seen
import collections.vecdeque
```

## SmallVec\<T\>

Inline-storage vector that avoids heap allocation for small sizes:

```seen
let sv = seen_small_vec_new(8)   // inline capacity of 8
seen_small_vec_push_i64(sv, 42)
let val = seen_small_vec_get_i64(sv, 0)
let len = seen_small_vec_length(sv)
seen_small_vec_clear(sv)
```

Falls back to heap when inline capacity is exceeded.

## StringHashMap\<V\>

Optimized `HashMap<String, V>` with string-specific hashing.

```seen
import collections.string_hash_map
```

## ByteBuffer

Low-level byte buffer for binary data.

```seen
import collections.byte_buffer
```

## Pool\<T\>

Object pool for reusing allocated objects.

```seen
import collections.pool
```
