#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPILER="${COMPILER:-$ROOT_DIR/compiler_seen/target/seen}"
if [[ "$COMPILER" != /* ]]; then
    if [[ "$COMPILER" == */* ]]; then
        COMPILER="$(cd "$(dirname "$COMPILER")" && pwd)/$(basename "$COMPILER")"
    else
        COMPILER="$(command -v "$COMPILER" || printf '%s' "$COMPILER")"
    fi
fi
TMP_DIR="$(mktemp -d /tmp/seen_trainer_blockers.XXXXXX)"

cleanup() {
    if [ -z "${SEEN_KEEP_TMP:-}" ]; then
        rm -rf "$TMP_DIR"
    else
        echo "KEEP: $TMP_DIR"
    fi
}

trap cleanup EXIT

apply_memory_cap() {
    if [ -n "${SEEN_TEST_NO_ULIMIT:-}" ]; then
        return
    fi

    local avail_kb current_limit target_kb
    avail_kb="$(awk '/MemAvailable/ { print $2 }' /proc/meminfo 2>/dev/null || echo 0)"
    if [ "$avail_kb" -le 0 ]; then
        return
    fi

    target_kb=$(( avail_kb / 2 ))
    if [ "$target_kb" -gt 8388608 ]; then
        target_kb=8388608
    fi
    if [ "$target_kb" -lt 1048576 ]; then
        return
    fi

    current_limit="$(ulimit -v)"
    if [ "$current_limit" = "unlimited" ] || [ "$current_limit" -gt "$target_kb" ]; then
        ulimit -v "$target_kb"
    fi
}

run_program() {
    local name="$1"
    local src="$2"
    local bin="$TMP_DIR/target/$name"
    local log="$TMP_DIR/$name.compile.log"
    local out="$TMP_DIR/$name.run.log"

    if ! "$COMPILER" compile "$src" "$bin" --fast --no-cache --no-fork \
        --jobs=1 --opt-jobs=1 >"$log" 2>&1; then
        echo "FAIL: $name did not compile"
        cat "$log"
        exit 1
    fi

    local status=0
    "$bin" >"$out" 2>&1 || status=$?
    if [ "$status" -ne 0 ]; then
        echo "FAIL: $name exited with $status"
        cat "$out"
        exit 1
    fi
}

apply_memory_cap
export SEEN_JOBS=1
export SEEN_OPT_JOBS=1

mkdir -p "$TMP_DIR/src/trainer" "$TMP_DIR/tests" "$TMP_DIR/target"
cat >"$TMP_DIR/Seen.toml" <<'TOML'
[project]
name = "trainer"
version = "0.1.0"
language = "en"
edition = "2025"

[build]
entry = "src/main.seen"
targets = ["native"]
TOML

cat >"$TMP_DIR/src/main.seen" <<'SEEN'
import trainer.jsonl.{tripletScore}

fun main() r: Int {
    if tripletScore("a", "bb", "ccc") == 6 {
        return 0
    }
    return 1
}
SEEN

cat >"$TMP_DIR/src/trainer/jsonl.seen" <<'SEEN'
pub fun tripletScore(a: String, p: String, n: String) r: Int {
    return a.length() + p.length() + n.length()
}
SEEN

cat >"$TMP_DIR/tests/test_jsonl.seen" <<'SEEN'
import trainer.jsonl.{tripletScore}

fun main() r: Int {
    if tripletScore("x", "yy", "zzz") == 6 {
        return 0
    }
    return 1
}
SEEN

(
    cd "$TMP_DIR"
    run_program "project_self_import" "tests/test_jsonl.seen"
)

mkdir -p "$TMP_DIR/file_range"
cat >"$TMP_DIR/file_range/main.seen" <<'SEEN'
import io.file.{readBytesRange, writeBytes, writeBytesAt}

fun expect(condition: Bool, code: Int) r: Int {
    if condition {
        return 0
    }
    return code
}

fun main() r: Int {
    let path = "/tmp/seen_trainer_blocker_range.bin"
    let bytes = Array<Int>()
    bytes.push(10)
    bytes.push(20)
    bytes.push(30)
    bytes.push(40)
    bytes.push(50)
    if not writeBytes(path, bytes) { return 1 }

    let patch = Array<Int>()
    patch.push(99)
    patch.push(100)
    if not writeBytesAt(path, 2, patch) { return 2 }

    let slice = readBytesRange(path, 1, 3)
    if expect(slice.length() == 3, 3) != 0 { return 3 }
    if expect(slice[0] == 20, 4) != 0 { return 4 }
    if expect(slice[1] == 99, 5) != 0 { return 5 }
    if expect(slice[2] == 100, 6) != 0 { return 6 }
    return 0
}
SEEN
run_program "file_range" "$TMP_DIR/file_range/main.seen"

mkdir -p "$TMP_DIR/string_runtime"
cat >"$TMP_DIR/string_runtime/main.seen" <<'SEEN'
fun main() r: Int {
    let lower = String.fromCharCode(72 + 32) + "ello"
    if lower.byteAt(0) != 104 { return 1 }
    if lower != "hello" { return 2 }

    let gpu = String.fromCharCode(103) + String.fromCharCode(112) + String.fromCharCode(117)
    if gpu != "gpu" { return 3 }
    if gpu.length() != 3 { return 4 }
    return 0
}
SEEN
run_program "string_runtime" "$TMP_DIR/string_runtime/main.seen"

mkdir -p "$TMP_DIR/argument_eval"
cat >"$TMP_DIR/argument_eval/main.seen" <<'SEEN'
var callCount: Int = 0

class Slot {
    var value: Int

    static fun new(value: Int) r: Slot {
        return Slot{value: value}
    }
}

fun makeSlot() r: Slot {
    callCount = callCount + 1
    return Slot.new(7)
}

class SlotMap {
    var total: Int

    static fun new() r: SlotMap {
        return SlotMap{total: 0}
    }

    fun addSlot(slot: Slot) r: Void {
        this.total = this.total + slot.value
    }
}

fun main() r: Int {
    let map = SlotMap.new()
    map.addSlot(makeSlot())
    if callCount != 1 { return callCount }
    if map.total != 7 { return 20 }
    return 0
}
SEEN
run_program "argument_eval" "$TMP_DIR/argument_eval/main.seen"

mkdir -p "$TMP_DIR/method_global_collision"
cat >"$TMP_DIR/method_global_collision/math_utils.seen" <<'SEEN'
pub fun relu(value: Float) r: Float {
    if value < 0.0 {
        return 0.0
    }
    return value
}
SEEN

cat >"$TMP_DIR/method_global_collision/main.seen" <<'SEEN'
import math_utils.{relu}

class Tensor {
    var values: Array<Float>

    static fun one(value: Float) r: Tensor {
        let values = Array<Float>()
        values.push(value)
        return Tensor{values: values}
    }

    fun relu() r: Tensor {
        let out = Array<Float>()
        out.push(relu(this.values[0]))
        return Tensor{values: out}
    }
}

fun main() r: Int {
    let tensor = Tensor.one(-2.0)
    let activated = tensor.relu()
    if activated.values[0] > -0.1 and activated.values[0] < 0.1 {
        return 0
    }
    return 1
}
SEEN
run_program "method_global_collision" "$TMP_DIR/method_global_collision/main.seen"

mkdir -p "$TMP_DIR/conditional_method"
cat >"$TMP_DIR/conditional_method/main.seen" <<'SEEN'
class LayerNormDeltaAdapter {
    var delta: Float

    static fun new(delta: Float) r: LayerNormDeltaAdapter {
        return LayerNormDeltaAdapter{delta: delta}
    }

    fun effectiveWeight(values: Array<Float>) r: Array<Float> {
        let out = Array<Float>()
        out.push(values[0] + this.delta)
        return out
    }

    fun effectiveBias(values: Array<Float>) r: Array<Float> {
        let out = Array<Float>()
        out.push(values[0] - this.delta)
        return out
    }
}

fun layerNormRows(input: Array<Float>, weight: Array<Float>, bias: Array<Float>) r: Array<Float> {
    let out = Array<Float>()
    out.push(input[0] + weight[0] + bias[0])
    return out
}

fun main() r: Int {
    let input = Array<Float>()
    input.push(1.0)
    let weight = Array<Float>()
    weight.push(2.0)
    let bias = Array<Float>()
    bias.push(3.0)

    let attentionLayerNormDelta = LayerNormDeltaAdapter.new(0.5)
    var attnNorm = layerNormRows(input, weight, bias)
    let applyDenseAdapter = true
    if applyDenseAdapter {
        let effectiveAttentionWeight = attentionLayerNormDelta.effectiveWeight(weight)
        let effectiveAttentionBias = attentionLayerNormDelta.effectiveBias(bias)
        attnNorm = layerNormRows(input, effectiveAttentionWeight, effectiveAttentionBias)
    }

    if attnNorm[0] > 5.9 and attnNorm[0] < 6.1 {
        return 0
    }
    return 1
}
SEEN
run_program "conditional_method" "$TMP_DIR/conditional_method/main.seen"

mkdir -p "$TMP_DIR/nested_arrays"
cat >"$TMP_DIR/nested_arrays/main.seen" <<'SEEN'
class Tokenizer {
    static fun new() r: Tokenizer {
        return Tokenizer{}
    }

    fun tokenIds(text: String) r: Array<Int> {
        let out = Array<Int>()
        out.push(text.length())
        out.push(text.byteAt(0))
        return out
    }

    fun batchTokenIds(texts: Array<String>) r: Array<Array<Int>> {
        let out = Array<Array<Int>>()
        var i = 0
        while i < texts.length() {
            out.push(this.tokenIds(texts[i]))
            i = i + 1
        }
        return out
    }
}

fun main() r: Int {
    let texts = Array<String>()
    texts.push("hi")
    texts.push("seen")
    let tokenizer = Tokenizer.new()
    let batch = tokenizer.batchTokenIds(texts)
    if batch.length() != 2 { return 1 }
    if batch[0][0] != 2 { return 2 }
    if batch[1][1] != 115 { return 3 }
    return 0
}
SEEN
run_program "nested_arrays" "$TMP_DIR/nested_arrays/main.seen"

mkdir -p "$TMP_DIR/class_array_return"
cat >"$TMP_DIR/class_array_return/main.seen" <<'SEEN'
class Adapter {
    var values: Array<Float>
    var loaded: Bool

    static fun loadedOne() r: Adapter {
        let values = Array<Float>()
        values.push(1.5)
        values.push(2.5)
        return Adapter{values: values, loaded: true}
    }

    fun summaryValue() r: Int {
        if this.loaded and this.values.length() == 2 {
            return 7
        }
        return 0
    }
}

class Entry {
    var name: String
    var rows: Int

    static fun new(name: String, rows: Int) r: Entry {
        return Entry{name: name, rows: rows}
    }
}

class Slot {
    var logical: String
    var rows: Int

    static fun resolved(logical: String, rows: Int) r: Slot {
        return Slot{logical: logical, rows: rows}
    }
}

class Info {
    var entries: Array<Entry>

    static fun new() r: Info {
        let entries = Array<Entry>()
        entries.push(Entry.new("embeddings.word_embeddings.weight", 384))
        return Info{entries: entries}
    }
}

fun loadAdapter() r: Adapter {
    return Adapter.loadedOne()
}

fun resolveSlot(info: Info, logicalName: String) r: Slot {
    let entry = info.entries[0]
    return Slot.resolved(logicalName, entry.rows)
}

fun main() r: Int {
    let adapter = loadAdapter()
    if adapter.summaryValue() != 7 { return 1 }
    if not adapter.loaded { return 2 }
    if adapter.values[1] < 2.4 or adapter.values[1] > 2.6 { return 3 }

    let info = Info.new()
    let slot = resolveSlot(info, "embeddings.word")
    if slot.rows != 384 { return 4 }
    if slot.logical != "embeddings.word" { return 5 }
    return 0
}
SEEN
run_program "class_array_return" "$TMP_DIR/class_array_return/main.seen"

mkdir -p "$TMP_DIR/json_parse"
cat >"$TMP_DIR/json_parse/main.seen" <<'SEEN'
import json.parser.{parseJson}

fun main() r: Int {
    let empty = parseJson("{}")
    if empty.success != 1 { return 1 }
    if empty.value.unwrap().length() != 0 { return 2 }

    let parsed = parseJson("{\"hidden_size\":16,\"backend\":\"gpu\"}")
    if parsed.success != 1 { return 3 }
    let root = parsed.value.unwrap()
    let hidden = root.get("hidden_size")
    if hidden.isNone() { return 4 }
    if hidden.unwrap().getInt() != 16 { return 5 }
    let backend = root.get("backend")
    if backend.isNone() { return 6 }
    if backend.unwrap().getString() != "gpu" { return 7 }
    return 0
}
SEEN
run_program "json_parse" "$TMP_DIR/json_parse/main.seen"

mkdir -p "$TMP_DIR/gpu_probe/src" "$TMP_DIR/gpu_probe/target"
cat >"$TMP_DIR/gpu_probe/Seen.toml" <<'TOML'
[project]
name = "gpu_probe"
version = "0.1.0"
language = "en"
edition = "2025"

[build]
entry = "src/main.seen"
targets = ["native"]
TOML

cat >"$TMP_DIR/gpu_probe/src/main.seen" <<'SEEN'
@compute(workgroup: 64)
fun vectorAdd(a: Buffer<Float>, b: Buffer<Float>, result: Buffer<Float>) {
    let idx = globalInvocationId.x
    result[idx] = a[idx] + b[idx]
}

fun main() r: Int {
    println("probe")
    return 0
}
SEEN

(
    cd "$TMP_DIR/gpu_probe"
    if ! "$COMPILER" compile src/main.seen target/probe --fast --no-cache \
        --no-fork --emit-glsl --jobs=1 --opt-jobs=1 \
        >"$TMP_DIR/gpu_probe.compile.log" 2>&1; then
        echo "FAIL: gpu_probe did not compile"
        cat "$TMP_DIR/gpu_probe.compile.log"
        exit 1
    fi
)

if [ ! -f "$TMP_DIR/gpu_probe/target/probe.shaders/vectorAdd.comp.glsl" ]; then
    echo "FAIL: gpu_probe did not emit GLSL"
    find "$TMP_DIR/gpu_probe" -maxdepth 4 -type f | sort
    exit 1
fi
if [ ! -f "$TMP_DIR/gpu_probe/target/probe.shaders/vectorAdd.reflect.json" ]; then
    echo "FAIL: gpu_probe did not emit reflection JSON"
    find "$TMP_DIR/gpu_probe" -maxdepth 4 -type f | sort
    exit 1
fi
if command -v glslc >/dev/null 2>&1 &&
    [ ! -f "$TMP_DIR/gpu_probe/target/probe.shaders/vectorAdd.comp.spv" ]; then
    echo "FAIL: gpu_probe did not emit SPIR-V even though glslc is available"
    find "$TMP_DIR/gpu_probe" -maxdepth 4 -type f | sort
    exit 1
fi

echo "PASS: trainer blocker regressions"
