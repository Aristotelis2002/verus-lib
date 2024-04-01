//! Properties of bitwise operators.
use builtin::*;
use builtin_macros::*;

verus! {

#[cfg(verus_keep_ghost)]
use crate::arithmetic::power2::{
    pow2,
    lemma_pow2_adds,
    lemma_pow2_pos,
    lemma2_to64,
    lemma_pow2_strictly_increases,
};
#[cfg(verus_keep_ghost)]
use crate::arithmetic::div_mod::lemma_div_denominator;
#[cfg(verus_keep_ghost)]
use crate::arithmetic::mul::{
    lemma_mul_inequality,
    lemma_mul_is_commutative,
    lemma_mul_is_associative,
};
#[cfg(verus_keep_ghost)]
use crate::calc_macro::*;

} // verus!
macro_rules! lemma_shr_is_div {
    ($name:ident, $name_auto:ident, $uN:ty) => {
        verus! {
        #[doc = "Proof that for given x and n of type "]
        #[doc = stringify!($uN)]
        #[doc = ", shifting x right by n is equivalent to division of x by 2^n."]
        pub proof fn $name(x: $uN, shift: $uN)
            requires
                0 <= shift < <$uN>::BITS,
            ensures
                x >> shift == x as nat / pow2(shift as nat),
            decreases shift,
        {
            reveal(pow2);
            if shift == 0 {
                assert(x >> 0 == x) by (bit_vector);
                assert(pow2(0) == 1) by (compute_only);
            } else {
                assert(x >> shift == (x >> ((sub(shift, 1)) as $uN)) / 2) by (bit_vector)
                    requires
                        0 < shift < <$uN>::BITS,
                ;
                calc!{ (==)
                    (x >> shift) as nat;
                        {}
                    ((x >> ((sub(shift, 1)) as $uN)) / 2) as nat;
                        { $name(x, (shift - 1) as $uN); }
                    (x as nat / pow2((shift - 1) as nat)) / 2;
                        {
                            lemma_pow2_pos((shift - 1) as nat);
                            lemma2_to64();
                            lemma_div_denominator(x as int, pow2((shift - 1) as nat) as int, 2);
                        }
                    x as nat / (pow2((shift - 1) as nat) * pow2(1));
                        {
                            lemma_pow2_adds((shift - 1) as nat, 1);
                        }
                    x as nat / pow2(shift as nat);
                }
            }
        }

        #[doc = "Proof that for all x and n of type "]
        #[doc = stringify!($uN)]
        #[doc = ", shifting x right by n is equivalent to division of x by 2^n."]
        pub proof fn $name_auto()
            ensures
                forall|x: $uN, shift: $uN|
                    0 <= shift < <$uN>::BITS ==> #[trigger] (x >> shift) == x as nat / pow2(shift as nat),
        {
            assert forall|x: $uN, shift: $uN| 0 <= shift < <$uN>::BITS implies #[trigger] (x >> shift) == x as nat
                / pow2(shift as nat) by {
                $name(x, shift);
            }
        }
        }
    };
}

lemma_shr_is_div!(lemma_u64_shr_is_div, lemma_u64_shr_is_div_auto, u64);
lemma_shr_is_div!(lemma_u32_shr_is_div, lemma_u32_shr_is_div_auto, u32);
lemma_shr_is_div!(lemma_u16_shr_is_div, lemma_u16_shr_is_div_auto, u16);
lemma_shr_is_div!(lemma_u8_shr_is_div, lemma_u8_shr_is_div_auto, u8);

verus! {

pub proof fn lemma_u64_pow2_no_overflow(n: nat)
    requires
        0 <= n < 64,
    ensures
        pow2(n) < 0x1_0000_0000_0000_0000,
{
    lemma_pow2_strictly_increases(n, 64);
    lemma2_to64();
}

pub proof fn lemma_u64_shl_is_mul(x: u64, shift: u64)
    requires
        0 <= shift < 64,
        x * pow2(shift as nat) < 0x1_0000_0000_0000_0000,
    ensures
        x << shift == x * pow2(shift as nat),
    decreases shift,
{
    lemma_u64_pow2_no_overflow(shift as nat);
    if shift == 0 {
        assert(x << 0 == x) by (bit_vector);
        assert(pow2(0) == 1) by (compute_only);
    } else {
        assert(x << shift == mul(x << ((sub(shift, 1)) as u64), 2)) by (bit_vector)
            requires
                0 < shift < 64,
        ;
        assert((x << (sub(shift, 1) as u64)) == x * pow2(sub(shift, 1) as nat)) by {
            lemma_pow2_strictly_increases((shift - 1) as nat, shift as nat);
            lemma_mul_inequality(
                pow2((shift - 1) as nat) as int,
                pow2(shift as nat) as int,
                x as int,
            );
            lemma_mul_is_commutative(x as int, pow2((shift - 1) as nat) as int);
            lemma_mul_is_commutative(x as int, pow2(shift as nat) as int);
            lemma_u64_shl_is_mul(x, (shift - 1) as u64);
        }
        calc!{ (==)
            ((x << (sub(shift, 1) as u64)) * 2);
                {}
            ((x * pow2(sub(shift, 1) as nat)) * 2);
                {
                    lemma_mul_is_associative(x as int, pow2(sub(shift, 1) as nat) as int, 2);
                }
            x * ((pow2(sub(shift, 1) as nat)) * 2);
                {
                    lemma_pow2_adds((shift - 1) as nat, 1);
                    lemma2_to64();
                }
            x * pow2(shift as nat);
        }
    }
}

} // verus!
