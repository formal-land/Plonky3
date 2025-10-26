# Rocq Print

An executable that prints the constraints of a Plonky3 circuit, in order to compare them to the constraints of a Rocq model.

For now, we only print the Keccak constraints.

## Run

From the root of the repository, run:

```
cargo run -p rocq_print
```

It will print the constraints of the Keccak circuit.
