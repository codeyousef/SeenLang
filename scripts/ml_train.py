#!/usr/bin/env python3
"""
Train ML weights for the Seen compiler's optimization heuristics.

Reads ml_training_data.tsv (produced by collect_ml_data.sh) and trains
a weighted linear model for inline decisions.

Output: .seen_ml_heuristics file with learned weights.

Usage:
    python3 scripts/ml_train.py [--input ml_training_data.tsv] [--output .seen_ml_heuristics]
"""

import sys
import os
import math
from collections import defaultdict

def parse_training_data(path):
    """Parse ml_training_data.tsv into structured records."""
    features = []  # list of (benchmark, category, funcName, f0, f1, f2, f3, f4)
    decisions = []  # list of (benchmark, category, action, detail)
    runtimes = {}  # benchmark -> runtime_ms
    llvm_remarks = []  # list of (benchmark, pass, name, func)

    with open(path) as f:
        header = f.readline()  # skip header
        for line in f:
            line = line.rstrip('\n')
            parts = line.split('\t')
            if len(parts) < 3:
                continue
            bench = parts[0]
            source = parts[1]
            content = parts[2] if len(parts) > 2 else ""

            if source == "runtime_ms":
                runtimes[bench] = int(content)
            elif source == "ml_decision":
                # Content spans parts[2:] since the decision log is tab-separated
                sub_parts = parts[2:]
                if len(sub_parts) >= 3:
                    if sub_parts[0] == "FEAT" and len(sub_parts) >= 8:
                        # FEAT\tcategory\tfuncName\tf0\tf1\tf2\tf3\tf4
                        features.append({
                            'benchmark': bench,
                            'category': sub_parts[1],
                            'funcName': sub_parts[2],
                            'f0': int(sub_parts[3]),
                            'f1': int(sub_parts[4]),
                            'f2': int(sub_parts[5]),
                            'f3': int(sub_parts[6]),
                            'f4': int(sub_parts[7]),
                        })
                    else:
                        decisions.append({
                            'benchmark': bench,
                            'category': sub_parts[0],
                            'action': sub_parts[1],
                            'detail': sub_parts[2] if len(sub_parts) > 2 else "",
                        })
            elif source == "llvm_remark":
                llvm_remarks.append({
                    'benchmark': bench,
                    'content': '\t'.join(parts[2:]),
                })

    return features, decisions, runtimes, llvm_remarks


def sigmoid(x):
    """Numerically stable sigmoid."""
    if x >= 0:
        return 1.0 / (1.0 + math.exp(-x))
    else:
        ez = math.exp(x)
        return ez / (1.0 + ez)


def train_inline_model(features, decisions, runtimes):
    """
    Train a logistic regression model for inline decisions.

    Features (per function):
        f0: bodySize
        f1: callCount
        f2: hasLoops (0/1)
        f3: nestingDepth (param count as proxy)
        f4: reserved

    Label: whether the function was inlined (from compiler decision + LLVM outcome).
    We use the compiler's own decisions as soft labels, weighted by benchmark speed.
    """
    # Collect inline feature vectors with their decisions
    inline_data = []
    for feat in features:
        if feat['category'] != 'inline':
            continue
        bench = feat['benchmark']
        func = feat['funcName']
        # Find the decision for this function
        label = 0.5  # default: no opinion
        for dec in decisions:
            if dec['benchmark'] == bench and dec['category'] == 'inline':
                detail = dec['detail']
                # detail format: "funcName:sz=N"
                if detail.startswith(func + ':'):
                    if dec['action'] == 'yes':
                        label = 1.0
                    elif dec['action'] == 'hint':
                        label = 0.7
                    elif dec['action'] == 'no':
                        label = 0.0
                    break

        inline_data.append({
            'features': [feat['f0'], feat['f1'], feat['f2'], feat['f3']],
            'label': label,
            'benchmark': bench,
        })

    if not inline_data:
        print("No inline training data found.")
        return None

    print(f"Training inline model on {len(inline_data)} examples...")

    # Normalize features
    n_features = 4
    feat_max = [1.0] * n_features
    for d in inline_data:
        for i in range(n_features):
            feat_max[i] = max(feat_max[i], abs(d['features'][i]) + 1)

    # Initialize weights
    weights = [0.0] * n_features
    bias = 0.0
    lr = 0.01
    reg = 0.001  # L2 regularization

    # Train with SGD
    best_weights = weights[:]
    best_bias = bias
    best_loss = float('inf')

    for epoch in range(200):
        total_loss = 0.0
        for d in inline_data:
            # Normalize
            x = [d['features'][i] / feat_max[i] for i in range(n_features)]
            y = d['label']

            # Forward
            z = sum(w * xi for w, xi in zip(weights, x)) + bias
            pred = sigmoid(z)

            # Binary cross-entropy loss
            eps = 1e-7
            loss = -(y * math.log(pred + eps) + (1 - y) * math.log(1 - pred + eps))
            total_loss += loss

            # Gradient
            grad = pred - y
            for i in range(n_features):
                weights[i] -= lr * (grad * x[i] + reg * weights[i])
            bias -= lr * grad

        avg_loss = total_loss / len(inline_data)
        if avg_loss < best_loss:
            best_loss = avg_loss
            best_weights = weights[:]
            best_bias = bias

    # Convert to fixed-point (scale by normalization factors)
    # The compiler uses raw features (not normalized), so we bake normalization into weights
    final_weights = [best_weights[i] / feat_max[i] for i in range(n_features)]

    print(f"  Loss: {best_loss:.4f}")
    print(f"  Weights: {[f'{w:.4f}' for w in final_weights]}")
    print(f"  Bias: {best_bias:.4f}")

    return final_weights, best_bias


def write_heuristics(path, inline_result, thresholds=None):
    """Write .seen_ml_heuristics config file."""
    with open(path, 'w') as f:
        f.write("# Seen ML Heuristics - auto-generated by ml_train.py\n")
        f.write("# Do not edit manually; re-run scripts/ml_train.py to regenerate\n")

        if thresholds:
            f.write(f"inline_threshold={thresholds.get('inline', 50)}\n")
            f.write(f"unroll_threshold={thresholds.get('unroll', 4)}\n")
            f.write(f"vectorize_threshold={thresholds.get('vectorize', 8)}\n")

        if inline_result:
            weights, bias = inline_result
            f.write(f"inline_w0={weights[0]:.4f}\n")
            f.write(f"inline_w1={weights[1]:.4f}\n")
            f.write(f"inline_w2={weights[2]:.4f}\n")
            f.write(f"inline_w3={weights[3]:.4f}\n")
            f.write(f"inline_bias={bias:.4f}\n")
            f.write("weights_active=1\n")

    print(f"Wrote: {path}")


def main():
    input_path = "ml_training_data.tsv"
    output_path = ".seen_ml_heuristics"

    # Parse args
    args = sys.argv[1:]
    i = 0
    while i < len(args):
        if args[i] == "--input" and i + 1 < len(args):
            input_path = args[i + 1]
            i += 2
        elif args[i] == "--output" and i + 1 < len(args):
            output_path = args[i + 1]
            i += 2
        else:
            i += 1

    if not os.path.exists(input_path):
        print(f"Error: {input_path} not found. Run scripts/collect_ml_data.sh first.")
        sys.exit(1)

    print(f"=== Seen ML Training Pipeline ===")
    print(f"Input:  {input_path}")
    print(f"Output: {output_path}")
    print()

    features, decisions, runtimes, llvm_remarks = parse_training_data(input_path)
    print(f"Loaded: {len(features)} feature vectors, {len(decisions)} decisions, "
          f"{len(runtimes)} benchmarks, {len(llvm_remarks)} LLVM remarks")

    # Show data distribution
    sources = defaultdict(int)
    for f in features:
        prefix = f['benchmark'].split('_')[0] if '_' in f['benchmark'] else f['benchmark']
        sources[prefix] += 1
    if sources:
        print("\nData distribution:")
        for src, count in sorted(sources.items(), key=lambda x: -x[1]):
            pct = count * 100.0 / len(features) if features else 0
            print(f"  {src}: {count} examples ({pct:.1f}%)")
    print()

    # Train inline model
    inline_result = train_inline_model(features, decisions, runtimes)

    # Write output
    write_heuristics(output_path, inline_result)

    print()
    print("Next step: scripts/ml_validate.sh")


if __name__ == "__main__":
    main()
