# gcc example

Benchmarks GCC compilation of `workload.c` across optimization levels.

## Setup

```
fossil project create gcc
cp compile.toml ~/.fossil/projects/gcc/fossils/compile/fossil.toml
cp analyze.py ~/.fossil/projects/gcc/fossils/compile/analyze.py
```

## Usage

Run from this directory (where `workload.c` lives):

```
fossil bury compile --variant O0 --project gcc
fossil bury compile --variant O2 --project gcc
fossil compare compile O0 O2 --project gcc
```
