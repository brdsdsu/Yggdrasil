# Yggdrasil: Yet Another 3D Scene Graph!

**Yggdrasil** is an ecosystem designed to power the perception infrastructure for autonomous systems. Whether deployed in a lab or across complex real-world environments, Yggdrasil provides the structural "backbone" needed to manage spatial data at scale.

The project consists of five core components:
1. **Scene Graph Library**: A pure Rust implementation providing the core API and data structure for high-performance spatial management.
2. **Engine**: The system runtime responsible for conflict resolution, lazy evaluation, and state updates.
3. **Service**: A distributed database implementation optimized for deployment across edge networks.
4. **Adapters**: SDKs that facilitate integration between Yggdrasil and external environments, such as `ROS2`.
5. **Python Bindings**: High-level wrappers providing seamless access to the Rust core for Python-based research, scripting, and rapid prototyping.
