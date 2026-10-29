"""Generate the deterministic IS-Net-like ONNX test fixture for kiln.

The real RMBG-1.4 weights are licence-gated (BRIA, non-commercial) and never
committed. This fixture stands in for the network in CI and sandbox tests:

- input  `input`  float32 [1, 3, 64, 64]   (same NCHW contract as RMBG-1.4)
- output `output` float32 [1, 1, 64, 64]   (same [1, 1, H, W] contract)

Graph: out = input[:, 0:1] * 2 - 1.
Channel 0 of the preprocessed tensor sits in [-1, 1], so the output regularly
exceeds [-1, 1] — exactly the property of real RMBG output that makes the
min-max matte normalisation load-bearing.
"""

import onnx
from onnx import TensorProto, helper, numpy_helper

INPUT = "input"
OUTPUT = "output"

graph = helper.make_graph(
    nodes=[
        helper.make_node("Slice", [INPUT, "starts", "ends", "axes"], ["ch0"]),
        helper.make_node("Mul", ["ch0", "two"], ["scaled"]),
        helper.make_node("Sub", ["scaled", "one"], [OUTPUT]),
    ],
    name="isnet_like",
    inputs=[helper.make_tensor_value_info(INPUT, TensorProto.FLOAT, [1, 3, 64, 64])],
    outputs=[helper.make_tensor_value_info(OUTPUT, TensorProto.FLOAT, [1, 1, 64, 64])],
    initializer=[
        numpy_helper.from_array(np_starts := __import__("numpy").array([0], dtype="int64"), "starts"),
        numpy_helper.from_array(__import__("numpy").array([1], dtype="int64"), "ends"),
        numpy_helper.from_array(__import__("numpy").array([1], dtype="int64"), "axes"),
        numpy_helper.from_array(__import__("numpy").array([2.0], dtype="float32"), "two"),
        numpy_helper.from_array(__import__("numpy").array([1.0], dtype="float32"), "one"),
    ],
)

model = helper.make_model(
    graph,
    opset_imports=[helper.make_opsetid("", 13)],
    producer_name="kiln-test-fixture",
)
model.ir_version = 8  # broad runtime compatibility

onnx.checker.check_model(model)
out = "assets/test/isnet_like.onnx"
onnx.save(model, out)
print("wrote", out)

import hashlib
digest = hashlib.sha256(open(out, "rb").read()).hexdigest()
print("sha256:", digest)
