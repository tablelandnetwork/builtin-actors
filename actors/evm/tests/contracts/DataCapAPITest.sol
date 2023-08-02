// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.17;

contract DataCapApiTest {
    uint8 constant MajTextString = 3;

    function sliceUInt8(
        bytes memory bs,
        uint start
    ) internal pure returns (uint8) {
        require(bs.length >= start + 1, "slicing out of range");
        return uint8(bs[start]);
    }

    function sliceUInt16(
        bytes memory bs,
        uint start
    ) internal pure returns (uint16) {
        require(bs.length >= start + 2, "slicing out of range");
        bytes2 x;
        assembly {
            x := mload(add(bs, add(0x20, start)))
        }
        return uint16(x);
    }

    /// @notice slice uint32 from bytes starting at a given index
    /// @param bs bytes to slice from
    /// @param start current position to slice from bytes
    /// @return uint32 sliced from bytes
    function sliceUInt32(
        bytes memory bs,
        uint start
    ) internal pure returns (uint32) {
        require(bs.length >= start + 4, "slicing out of range");
        bytes4 x;
        assembly {
            x := mload(add(bs, add(0x20, start)))
        }
        return uint32(x);
    }

    function sliceUInt64(
        bytes memory bs,
        uint start
    ) internal pure returns (uint64) {
        require(bs.length >= start + 8, "slicing out of range");
        bytes8 x;
        assembly {
            x := mload(add(bs, add(0x20, start)))
        }
        return uint64(x);
    }

    function parseCborHeader(
        bytes memory cbor,
        uint byteIndex
    ) internal pure returns (uint8, uint64, uint) {
        uint8 first = sliceUInt8(cbor, byteIndex);
        byteIndex += 1;
        uint8 maj = (first & 0xe0) >> 5;
        uint8 low = first & 0x1f;
        // We don't handle CBOR headers with extra > 27, i.e. no indefinite lengths
        require(low < 28, "cannot handle headers with extra > 27");

        // extra is lower bits
        if (low < 24) {
            return (maj, low, byteIndex);
        }

        // extra in next byte
        if (low == 24) {
            uint8 next = sliceUInt8(cbor, byteIndex);
            byteIndex += 1;
            require(next >= 24, "invalid cbor"); // otherwise this is invalid cbor
            return (maj, next, byteIndex);
        }

        // extra in next 2 bytes
        if (low == 25) {
            uint16 extra16 = sliceUInt16(cbor, byteIndex);
            byteIndex += 2;
            return (maj, extra16, byteIndex);
        }

        // extra in next 4 bytes
        if (low == 26) {
            uint32 extra32 = sliceUInt32(cbor, byteIndex);
            byteIndex += 4;
            return (maj, extra32, byteIndex);
        }

        // extra in next 8 bytes
        assert(low == 27);
        uint64 extra64 = sliceUInt64(cbor, byteIndex);
        byteIndex += 8;
        return (maj, extra64, byteIndex);
    }

    function deserializeResponse(
        bytes memory ret_val
    ) internal pure returns (string memory) {
        string memory response;
        uint byteIdx = 0;

        // --- parsing CBOR header ---
        uint8 maj;
        uint len;

        (maj, len, byteIdx) = parseCborHeader(ret_val, byteIdx);
        require(maj == MajTextString, "invalid maj (expected MajTextString)");

        // --
        uint max_len = byteIdx + len;
        bytes memory slice = new bytes(len);
        uint slice_index = 0;
        for (uint256 i = byteIdx; i < max_len; i++) {
            slice[slice_index] = ret_val[i];
            slice_index++;
        }

        response = string(slice);
        byteIdx += len;

        return response;
    }

    function symbol() public returns (string memory) {
        // hardcode for testing
        // 1.
        bytes memory raw_request = new bytes(0);
        // 2.
        address CALL_ACTOR_ID = 0xfe00000000000000000000000000000000000005;
        // 3.
        uint256 value = 0;
        // 4.
        uint256 method_num = 2481995435; // Tableland method Symbol (Ping hash)
        // 5.
        bool static_call = true;
        // 6.
        uint64 codec = 0x00; //Misc.NONE_CODEC
        // 7.
        // CommonTypes.FilActorId target = DataCapTypes.ActorID;

        (bool success, bytes memory data) = address(CALL_ACTOR_ID).delegatecall(
            abi.encode(
                uint64(method_num),
                value,
                static_call,
                codec,
                raw_request,
                17
            )
        );

        require(success, "call to DataCap actor failed");

        (int256 exit, uint64 return_codec, bytes memory return_value) = abi
            .decode(data, (int256, uint64, bytes));

        if (return_codec == 0x00) {
            // Misc.NONE_CODEC
            if (return_value.length != 0) {
                revert("error");
            }
        } else if (
            return_codec == 0x51 || // Misc.CBOR_CODEC
            return_codec == 0x71 // Misc.DAG_CBOR_CODEC
        ) {
            if (return_value.length == 0) {
                revert("error");
            }
        } else {
            revert("error");
        }

        require(exit == 0, "DataCap actor returned an error");

        return deserializeResponse(return_value);
    }
}
