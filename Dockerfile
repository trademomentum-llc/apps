FROM rust:1.89.0-bookworm@sha256:948f9b08a66e7fe01b03a98ef1c7568292e07ec2e4fe90d88c07bb14563c84ff

# Install runtime/build dependencies without bootstrap scripts.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy source code
COPY . .

# Build the project
RUN cargo build --locked --release

# Test the return42 binary
RUN ./target/release/morphlex tokenize "return 42"

# Create a simple test
RUN echo "return 42" > test.jstr
RUN ./target/release/morphlex compile test.jstr -o test_output.bin
RUN chmod +x test_output.bin
RUN ./test_output.bin
RUN echo "Exit code: $?"
