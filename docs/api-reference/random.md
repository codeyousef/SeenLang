# Random

Three random number generators are available, each with different tradeoffs.

```seen
import random.rng
```

## LCG (Linear Congruential Generator)

Fastest, lowest quality. Good for non-critical randomness.

```seen
let rng = LCG.new(seed)
let value = rng.next()
```

## PCG (Permuted Congruential Generator)

Good balance of speed and quality. Recommended for general use.

```seen
let rng = PCG.new(seed)
let value = rng.next()
```

## Xorshift

Simple and fast with reasonable quality.

```seen
let rng = Xorshift.new(seed)
let value = rng.next()
```

## Example

```seen
fun main() {
    let rng = PCG.new(42)
    var i = 0
    while i < 10 {
        println("Random: {rng.next()}")
        i = i + 1
    }
}
```

## UWW Deterministic Random

For deterministic/WASM contexts:

```seen
import uww

let rand = uww_deterministic_rand(seed)
```
