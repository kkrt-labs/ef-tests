%lang starknet

from starkware.cairo.common.uint256 import (
    Uint256,
    uint256_mul_div_mod,
)
from starkware.cairo.common.cairo_builtins import HashBuiltin, BitwiseBuiltin


@storage_var
func counter() -> (value: felt){
}

@external
func inc { 
    syscall_ptr: felt*,
    pedersen_ptr: HashBuiltin*,
    range_check_ptr,
    bitwise_ptr: BitwiseBuiltin*
}(){
   let (current_counter) = counter.read();
   counter.write(current_counter + 1);
   return();
}
