# Neural Network-based ML Optimizer for Seen Compiler

This module provides a machine learning-based optimization guidance system for the Seen compiler, replacing threshold-based heuristics with neural networks that learn from real compilation outcomes.

## Overview

The ML Optimizer uses neural networks to make intelligent decisions about:
- **Loop Vectorization**: Whether to vectorize a loop based on its characteristics
- **Loop Unrolling**: Whether to unroll a loop and by what factor
- **Function Inlining**: Whether to inline a function call

## Architecture

### Components

1. **NeuralNetwork** (`neural_network.seen`)
   - Feedforward neural network with configurable layers
   - Supports ReLU and Sigmoid activations
   - Backpropagation training with configurable learning rate
   - Model save/load functionality

2. **TrainingData** (`training_data.seen`)
   - Collects and manages training examples
   - Tracks optimization decisions and their outcomes
   - Processes LLVM optimization remarks
   - Provides train/validation data splitting

3. **MLOptimizerNN** (`ml_optimizer_nn.seen`)
   - Main optimizer interface using neural networks
   - Falls back to heuristics when NN confidence is low
   - Decision caching for performance
   - Online learning from LLVM remarks

4. **Mod** (`mod_nn.seen`)
   - Unified module interface
   - Backwards compatibility layer

## Usage

### Basic Usage

```seen
import optimization.ml.{MLOptimizerNN, LoopFeaturesNN, InlineFeaturesNN}

// Create optimizer
let optimizer = MLOptimizerNN.new()
optimizer.setTargetHardware("x86_64")

// Make vectorization decision
let loopFeatures = LoopFeaturesNN.new(
    tripCount: 100,
    nestingDepth: 1,
    addOps: 10,
    mulOps: 2,
    loadOps: 5,
    storeOps: 3,
    stride: 1,
    sourceLocation: "myfile.seen:42",
    functionName: "processArray"
)

if optimizer.shouldVectorize(loopFeatures) {
    // Apply vectorization
    applyVectorization()
}

// Make inlining decision
let inlineFeatures = InlineFeaturesNN.new(
    calleeSize: 15,
    callerSize: 200,
    callCount: 5,
    hasLoops: false,
    sourceLocation: "myfile.seen:50",
    functionName: "helperFunc"
)

if optimizer.shouldInline(inlineFeatures) {
    // Apply inlining
    applyInlining()
}
```

### Training the Models

```seen
// Train with collected data
let result = optimizer.trainModels(epochs: 100)

if result.success {
    println("Training complete!")
    println("Accuracy: " + result.accuracy.toString())
    println("Loss: " + result.loss.toString())
}

// Save trained models
optimizer.saveModels()
```

### Processing LLVM Remarks

```seen
// After compilation, process LLVM optimization remarks
let remarks = collectLLVMRemarks()
optimizer.processOptimizationRemarks(remarks)

// This updates the training data with actual outcomes
// and performs online learning to improve the model
```

## Neural Network Architecture

### Loop Optimization Network
- **Input Layer**: 7 features
  - tripCount (normalized)
  - nestingDepth (normalized)
  - addOps (normalized)
  - mulOps (normalized)
  - loadOps (normalized)
  - storeOps (normalized)
  - stride (normalized)

- **Hidden Layers**: [16, 8] neurons
  - Layer 1: 16 neurons with ReLU
  - Layer 2: 8 neurons with ReLU

- **Output Layer**: 4 neurons with Sigmoid
  - 0: Vectorize
  - 1: Unroll
  - 2: Inline (for loop functions)
  - 3: No optimization

### Inline Decision Network
- **Input Layer**: 4 features (padded to 7)
  - calleeSize (normalized)
  - callerSize (normalized)
  - callCount (normalized)
  - hasLoops (0 or 1)

- **Hidden Layers**: [12, 6] neurons
  - Layer 1: 12 neurons with ReLU
  - Layer 2: 6 neurons with ReLU

- **Output Layer**: 4 neurons with Sigmoid

## Feature Normalization

Features are normalized to [0, 1] range:

| Feature | Min | Max |
|---------|-----|-----|
| tripCount | 0 | 10000 |
| nestingDepth | 0 | 5 |
| addOps | 0 | 100 |
| mulOps | 0 | 50 |
| loadOps | 0 | 50 |
| storeOps | 0 | 50 |
| stride | 0 | 100 |
| calleeSize | 0 | 500 |
| callerSize | 0 | 1000 |
| callCount | 0 | 100 |

## Training Process

### Initial Training
1. Collect initial training data using heuristic decisions
2. Train both networks on the collected data
3. Validate on held-out validation set (20% split)
4. Enable NN when accuracy > 70%

### Online Learning
1. Make decisions using current model
2. Record actual outcomes from LLVM remarks
3. Update training data with outcomes
4. Perform incremental training on new examples
5. Retrain full model every 100 new examples

### Feedback Loop
```
Features → NN Prediction → Decision → LLVM Optimization
                                              ↓
Training ← Outcome Analysis ← LLVM Remarks ←┘
```

## Configuration

### Confidence Threshold
```seen
// Set minimum confidence to use NN decision (default: 0.7)
optimizer.setConfidenceThreshold(0.8)
```

### Learning Rate
```seen
// Set learning rate for training (default: 0.01)
optimizer.setLearningRate(0.005)
```

### Enable/Disable Neural Network
```seen
// Force use of heuristics
optimizer.enableNeuralNetwork(false)

// Enable NN when confidence is sufficient
optimizer.enableNeuralNetwork(true)
```

## File Structure

```
compiler_seen/src/optimization/ml/
├── neural_network.seen      # Neural network implementation
├── training_data.seen        # Training data management
├── ml_optimizer_nn.seen      # Main optimizer with NN
├── mod_nn.seen               # Module interface
├── test_nn_optimizer.seen    # Comprehensive tests
└── README_NN.md              # This documentation
```

## Model Persistence

Models are saved in JSON format to `.seen_cache/ml_models/`:
- `loop_network.json`: Loop optimization network weights
- `inline_network.json`: Inline decision network weights
- `ml_training_data.jsonl`: Training examples in JSONL format

## Testing

Run the test suite:

```seen
import optimization.ml.test_nn_optimizer

test_nn_optimizer.runAllTests()
```

Tests cover:
- Neural network operations (sigmoid, ReLU, forward/backward pass)
- Training data collection and conversion
- Decision making (heuristics and NN)
- Caching mechanisms
- End-to-end training workflow

## Performance Considerations

### Decision Caching
- Decisions are cached based on feature hash
- Significant speedup for repeated patterns
- Cache hit rate tracked in statistics

### Lazy Model Loading
- Models loaded only when needed
- Falls back to heuristics if loading fails

### Incremental Training
- Online learning for single examples
- Batch retraining only when data threshold reached

## Future Enhancements

1. **Deep Learning**: Experiment with deeper architectures
2. **Reinforcement Learning**: Use RL for exploration vs exploitation
3. **Multi-Objective**: Optimize for size vs speed trade-offs
4. **Hardware-Specific**: Train separate models per target architecture
5. **Feature Engineering**: Add more sophisticated IR features

## References

- Inspired by MLGO (Machine Learning for Compiler Optimization)
- Uses techniques from Auto-vectorization research
- Incorporates ideas from profile-guided optimization
