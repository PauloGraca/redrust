#!/usr/bin/env python3
"""
RedRust Benchmark Tool
Tests performance with persistent connection and pipelining
"""

import socket
import time
import statistics
from concurrent.futures import ThreadPoolExecutor, as_completed

HOST = "127.0.0.1"
PORT = 6379

def send_command(sock, cmd):
    """Send a single command and read response"""
    sock.sendall(cmd.encode() + b"\r\n")
    # Read until \r\n for simple responses
    response = b""
    while b"\r\n" not in response:
        data = sock.recv(4096)
        if not data:
            break
        response += data
    return response.decode()

def benchmark_set(iterations=1000):
    """Benchmark SET operations"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((HOST, PORT))
    
    start = time.time()
    for i in range(iterations):
        send_command(sock, f"SET benchmark:key:{i} value{i}")
    elapsed = time.time() - start
    
    sock.close()
    return iterations / elapsed

def benchmark_get(iterations=1000):
    """Benchmark GET operations"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((HOST, PORT))
    
    start = time.time()
    for i in range(iterations):
        send_command(sock, f"GET benchmark:key:{i}")
    elapsed = time.time() - start
    
    sock.close()
    return iterations / elapsed

def benchmark_lpush(iterations=1000):
    """Benchmark LPUSH operations"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((HOST, PORT))
    
    start = time.time()
    for i in range(iterations):
        send_command(sock, f"LPUSH benchmark:list item{i}")
    elapsed = time.time() - start
    
    sock.close()
    return iterations / elapsed

def benchmark_mixed(iterations=1000):
    """Benchmark mixed SET/GET operations"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((HOST, PORT))
    
    start = time.time()
    for i in range(iterations // 2):
        send_command(sock, f"SET mixed:key:{i} value{i}")
        send_command(sock, f"GET benchmark:key:{i}")
    elapsed = time.time() - start
    
    sock.close()
    return iterations / elapsed

def benchmark_parallel(operations, thread_count=10):
    """Run benchmarks in parallel threads"""
    latencies = []
    
    def worker(ops_per_thread):
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((HOST, PORT))
        
        local_latencies = []
        for i in range(ops_per_thread):
            start = time.time()
            send_command(sock, f"PING")
            elapsed = time.time() - start
            local_latencies.append(elapsed * 1000)  # Convert to ms
        
        sock.close()
        return local_latencies
    
    ops_per_thread = operations // thread_count
    
    with ThreadPoolExecutor(max_workers=thread_count) as executor:
        futures = [executor.submit(worker, ops_per_thread) for _ in range(thread_count)]
        for future in as_completed(futures):
            latencies.extend(future.result())
    
    return latencies

def cleanup():
    """Clean up benchmark keys"""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((HOST, PORT))
        sock.sendall(b"KEYS\r\n")
        response = b""
        while b"\r\n" not in response:
            data = sock.recv(4096)
            if not data:
                break
            response += data
        
        # Parse response and delete all benchmark:* keys
        import re
        keys = re.findall(r'\$\d+\r\n(benchmark:[^\r]+)', response.decode())
        for key in keys[:100]:  # Limit cleanup
            send_command(sock, f"DEL {key}")
        
        sock.close()
        print(f"Cleaned up {len(keys)} benchmark keys")
    except Exception as e:
        print(f"Cleanup warning: {e}")

def main():
    print("=" * 60)
    print("🦀 RedRust Benchmark Tool")
    print("=" * 60)
    print(f"Connecting to {HOST}:{PORT}\n")
    
    # Test connectivity
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((HOST, PORT))
        sock.sendall(b"PING\r\n")
        response = sock.recv(1024).decode()
        if "PONG" not in response:
            print("Error: Server not responding correctly")
            return
        sock.close()
        print("✓ Server is responsive\n")
    except Exception as e:
        print(f"Error: Cannot connect to server: {e}")
        print("Make sure RedRust is running: cargo run")
        return
    
    # Run benchmarks
    tests = [
        ("SET", benchmark_set, 1000),
        ("GET", benchmark_get, 1000),
        ("LPUSH", benchmark_lpush, 1000),
        ("Mixed (50% SET, 50% GET)", benchmark_mixed, 1000),
    ]
    
    print("Running single-threaded benchmarks:")
    print("-" * 60)
    
    for name, func, iterations in tests:
        print(f"\n{name} ({iterations} operations)...")
        try:
            ops_per_sec = func(iterations)
            print(f"  Result: {ops_per_sec:,.0f} req/sec")
        except Exception as e:
            print(f"  Error: {e}")
    
    # Latency test
    print("\n" + "-" * 60)
    print("Latency test (PING, 10 threads, 100 ops each)...")
    
    try:
        latencies = benchmark_parallel(1000, thread_count=10)
        print(f"  Min latency: {min(latencies):.2f} ms")
        print(f"  Max latency: {max(latencies):.2f} ms")
        print(f"  Avg latency: {statistics.mean(latencies):.2f} ms")
        print(f"  P50 latency: {statistics.median(latencies):.2f} ms")
        print(f"  P95 latency: {sorted(latencies)[int(len(latencies)*0.95)]:.2f} ms")
    except Exception as e:
        print(f"  Error: {e}")
    
    # Cleanup
    print("\n" + "=" * 60)
    cleanup()
    
    print("\n✅ Benchmark complete!")
    print("\nNote: These are basic benchmarks. For production-grade")
    print("      testing, use redis-benchmark or memtier_benchmark")

if __name__ == "__main__":
    main()
