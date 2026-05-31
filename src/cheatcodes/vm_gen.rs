//! Generate Vm.sol from our Rust cheatcode definitions.
//!
//! Running `lancer-foundry forge vm-sol` writes `Vm.sol` to the current directory.
//! This keeps Solidity and Rust in sync (the Rust selectors are the source of truth).

use crate::cheatcodes::selectors;

struct Cheatcode {
    sig: &'static str,
    doc: &'static str,
}

const CHEATCODES: &[Cheatcode] = &[
    Cheatcode { sig: "warp(uint256 timestamp)",         doc: "Set block.timestamp" },
    Cheatcode { sig: "roll(uint256 blockNumber)",       doc: "Set block.number" },
    Cheatcode { sig: "fee(uint256 baseFee)",            doc: "Set block.basefee" },
    Cheatcode { sig: "coinbase(address who)",           doc: "Set block.coinbase" },
    Cheatcode { sig: "chainId(uint256 id)",             doc: "Set chain id" },
    Cheatcode { sig: "prevrandao(bytes32 val)",         doc: "Set block.prevrandao" },
    Cheatcode { sig: "txGasPrice(uint256 gp)",         doc: "Set tx.gasprice" },
    Cheatcode { sig: "getBlockNumber() returns (uint256)", doc: "Get block.number" },
    Cheatcode { sig: "getBlockTimestamp() returns (uint256)", doc: "Get block.timestamp" },
    Cheatcode { sig: "getChainId() returns (uint256)",  doc: "Get chain id" },
    Cheatcode { sig: "deal(address who, uint256 amount)", doc: "Set ETH balance" },
    Cheatcode { sig: "etch(address who, bytes calldata code)", doc: "Set code at address" },
    Cheatcode { sig: "store(address target, bytes32 slot, bytes32 value)", doc: "Set storage slot" },
    Cheatcode { sig: "load(address target, bytes32 slot) returns (bytes32)", doc: "Read storage slot" },
    Cheatcode { sig: "setNonce(address account, uint64 nonce)", doc: "Set account nonce" },
    Cheatcode { sig: "getNonce(address account) returns (uint64)", doc: "Get account nonce" },
    Cheatcode { sig: "addr(uint256 privateKey) returns (address)", doc: "Derive address from key" },
    Cheatcode { sig: "prank(address sender)",          doc: "Next call is from sender" },
    Cheatcode { sig: "prank(address sender, address origin)", doc: "Next call with origin" },
    Cheatcode { sig: "startPrank(address sender)",     doc: "All calls are from sender" },
    Cheatcode { sig: "startPrank(address sender, address origin)", doc: "All calls with origin" },
    Cheatcode { sig: "stopPrank()",                    doc: "Stop prank" },
    Cheatcode { sig: "mockCall(address target, bytes calldata data, bytes calldata retData)", doc: "Mock a call" },
    Cheatcode { sig: "mockCallRevert(address target, bytes calldata data, bytes calldata revertData)", doc: "Mock a revert" },
    Cheatcode { sig: "clearMockedCalls()",             doc: "Clear all mocks" },
    Cheatcode { sig: "expectRevert()",                 doc: "Expect any revert" },
    Cheatcode { sig: "expectRevert(bytes calldata revertData)", doc: "Expect specific revert data" },
    Cheatcode { sig: "expectRevert(bytes4 selector)",  doc: "Expect custom error" },
    Cheatcode { sig: "expectEmit(bool t1, bool t2, bool t3, bool data)", doc: "Expect log" },
    Cheatcode { sig: "expectEmit(bool t1, bool t2, bool t3, bool data, address emitter)", doc: "Expect log from address" },
    Cheatcode { sig: "expectEmit()",                   doc: "Expect any log" },
    Cheatcode { sig: "expectCall(address target, bytes calldata data)", doc: "Expect a call" },
    Cheatcode { sig: "assume(bool condition)",         doc: "Discard fuzz input if false" },
    Cheatcode { sig: "skip(bool skip)",                doc: "Skip test if true" },
    Cheatcode { sig: "recordLogs()",                   doc: "Start recording logs" },
    Cheatcode { sig: "getRecordedLogs() returns (uint256)", doc: "Get recorded logs count" },
    Cheatcode { sig: "snapshotState() returns (uint256)", doc: "Snapshot EVM state" },
    Cheatcode { sig: "revertToState(uint256 id)",      doc: "Revert to snapshot" },
    Cheatcode { sig: "revertToStateAndDelete(uint256 id)", doc: "Revert and delete snapshot" },
    Cheatcode { sig: "pauseGasMetering()",             doc: "Pause gas metering" },
    Cheatcode { sig: "resumeGasMetering()",            doc: "Resume gas metering" },
    Cheatcode { sig: "setEnv(string calldata key, string calldata value)", doc: "Set env var" },
    Cheatcode { sig: "envString(string calldata key) returns (string memory)", doc: "Read string env var" },
    Cheatcode { sig: "envBool(string calldata key) returns (bool)", doc: "Read bool env var" },
    Cheatcode { sig: "envUint(string calldata key) returns (uint256)", doc: "Read uint env var" },
    Cheatcode { sig: "envAddress(string calldata key) returns (address)", doc: "Read address env var" },
    Cheatcode { sig: "envBytes32(string calldata key) returns (bytes32)", doc: "Read bytes32 env var" },
    Cheatcode { sig: "readFile(string calldata path) returns (string memory)", doc: "Read file contents" },
    Cheatcode { sig: "writeFile(string calldata path, string calldata data)", doc: "Write file" },
    Cheatcode { sig: "exists(string calldata path) returns (bool)", doc: "Check path exists" },
    Cheatcode { sig: "label(address addr, string calldata lbl)", doc: "Label address" },
    Cheatcode { sig: "getLabel(address addr) returns (string memory)", doc: "Get address label" },
    Cheatcode { sig: "ffi(string[] calldata commandInput) returns (bytes memory)", doc: "Run external command" },
    Cheatcode { sig: "projectRoot() returns (string memory)", doc: "Get project root" },
    Cheatcode { sig: "randomUint() returns (uint256)", doc: "Random uint256" },
    Cheatcode { sig: "randomAddress() returns (address)", doc: "Random address" },
    Cheatcode { sig: "sign(uint256 privateKey, bytes32 digest) returns (uint8, bytes32, bytes32)", doc: "Sign digest" },
];

pub fn generate_vm_sol() -> String {
    let mut out = String::from(
        "// SPDX-License-Identifier: MIT\n\
         // Generated by lancer-foundry — DO NOT EDIT\n\
         pragma solidity >=0.6.2 <0.9.0;\n\n\
         interface Vm {\n\
             address constant VM_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;\n\n",
    );

    for cc in CHEATCODES {
        let fn_name = cc.sig.split('(').next().unwrap_or("");
        out.push_str(&format!("    /// @dev {}\n", cc.doc));
        out.push_str(&format!("    function {} external;\n\n", cc.sig));
    }

    out.push_str("}\n");
    out
}
