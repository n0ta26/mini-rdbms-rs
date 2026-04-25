# mini-rdbms-rs
`mini-rdbms-rs` is a small relational database management system written in Rust.

This project is created for learning how an RDBMS works internally, including storage management, buffer management, SQL parsing, query execution, and transaction control.

## Concept
The goal of this project is not to build a production-ready database, but to understand the internal architecture of an RDBMS by implementing each component step by step.

Modern relational databases such as SQLite, PostgreSQL, and MySQL contain many sophisticated mechanisms.  

This project aims to implement a simplified version of those mechanisms with a clean and understandable design.

## Target Archtecture
This project will gradually implement the following components:

```text
SQL Client / CLI
      |
      v
SQL Parser
      |
      v
Planner / Optimizer
      |
      v
Executor
      |
      v
Access Method
      |
      v
Buffer Manager
      |
      v
Disk / Storage Manager
```

## Development Policy

This project will be developed from the bottom layer upward.

The implementation starts with the lowest-level components, such as the disk/storage manager, and gradually builds higher-level components such as the buffer manager, access methods, query executor, SQL parser, and interactive CLI.

This bottom-up approach is intended to help understand how each RDBMS component depends on the layers below it.

```text
SQL Client / CLI
      ^
      |
SQL Parser
      ^
      |
Planner / Optimizer
      ^
      |
Executor
      ^
      |
Access Method
      ^
      |
Buffer Manager
      ^
      |
Disk / Storage Manager
```

## Motivation

This project is developed as a personal learning project alongside my main work.

Because of that, the development pace may be gradual, and features will be implemented step by step as time allows.

The main motivation is to deepen my understanding of RDBMS internals by implementing each component from scratch.

## Development Environment

This project uses Docker Compose and Dev Containers to provide a reproducible Rust development environment.

The main tools are:

- Rust
- Cargo
- Docker Compose
- Dev Containers
- Visual Studio Code

Rust and Cargo are installed inside the development container, so the host machine does not need to manage the Rust toolchain directly.

Docker Compose is used to define the local development services, and Dev Containers are used to open the project as a consistent workspace in Visual Studio Code.

## CI/CD

This project uses GitHub Actions for CI/CD.

GitHub Actions will be used to automate checks such as formatting, linting, building, and testing.

The initial CI workflow will focus on verifying the Rust project with Cargo commands.

Planned checks include:

- `cargo fmt`
- `cargo clippy`
- `cargo build`
- `cargo test`