//! Complete outer-frame fingerprints and chained unwind partitions.
use super::scheduler::{FixedFrame, Fragment};
pub(super) const FRAMES: &[FixedFrame] = &[
    FixedFrame { begin: 0x35800, end: 0x35902,
        sha: "394a0f68ac126f5cbef05a18eee6ec6614c65122b8eca14b7665e88f1c48e86e",
        #[cfg(test)] specimen: "", parts: &[
        Fragment { begin: 0x35800, end: 0x35902, unwind: 0xacae3c, hex: "010a04000a3406000a320670" },
    ]},
    FixedFrame { begin: 0x383e0, end: 0x3882c,
        sha: "f6accf13def94cb1adb7ccf2915bd8cc33efa5c5c251a776d5e1d14450608da5",
        #[cfg(test)] specimen: "", parts: &[
        Fragment { begin: 0x383e0, end: 0x38402, unwind: 0xac3b64, hex: "011304001301390005700450" },
        Fragment { begin: 0x38402, end: 0x38822, unwind: 0xac3b70, hex: "211504001564380004343e00e083030002840300643bac00" },
        Fragment { begin: 0x38822, end: 0x3882c, unwind: 0xac3b88, hex: "21000000e083030002840300643bac00" },
    ]},
    FixedFrame { begin: 0x39650, end: 0x397b9,
        sha: "72e8c48964cd5c4e2fd8e53833c3f7ebe7844b0abc5d17d2956181a67842a033",
        #[cfg(test)] specimen: "", parts: &[
        Fragment { begin: 0x39650, end: 0x3967a, unwind: 0xad76d4, hex: "0106020006520250" },
        Fragment { begin: 0x3967a, end: 0x397b3, unwind: 0xac3bcc, hex: "212e08002ee40400117405000c640a0005340900509603007a960300d476ad00" },
        Fragment { begin: 0x397b3, end: 0x397b9, unwind: 0xac3bec, hex: "21000000509603007a960300d476ad00" },
    ]},
    FixedFrame { begin: 0x413b70, end: 0x414b1c,
        sha: "c0c7856976408660aad9a98187cb063e7abdc01147a55cef8fbea57faec65010",
        #[cfg(test)] specimen: "", parts: &[
        Fragment { begin: 0x413b70, end: 0x413ba0, unwind: 0xafef44, hex: "0118040018014d000ae00850" },
        Fragment { begin: 0x413ba0, end: 0x414b11, unwind: 0xafef50, hex: "21220c0022f4480014d4490010c44a000c744b0008644c0004345200703b4100a03b410044efaf00" },
        Fragment { begin: 0x414b11, end: 0x414b1c, unwind: 0xafef78, hex: "21000000703b4100a03b410044efaf00" },
    ]},
    FixedFrame { begin: 0x41a400, end: 0x41a42f,
        sha: "5cd428f77bcb706b431a89c3d644ffadbf3ffbdce1fd27fb66459954eb3ecf3a",
        #[cfg(test)] specimen: "", parts: &[
        Fragment { begin: 0x41a400, end: 0x41a42f, unwind: 0xacae3c, hex: "010a04000a3406000a320670" },
    ]},
];
