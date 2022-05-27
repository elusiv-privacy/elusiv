use crate::fields::fr_to_u256_le;
use crate::{macros::elusiv_account, fields::u256_to_fr};
use crate::types::U256;
use crate::bytes::*;
use super::program_account::*;
use borsh::{BorshDeserialize, BorshSerialize};
use ark_bn254::Fr;
use ark_ff::{BigInteger256, Zero};

/// Height of the active Merkle Tree
pub const MT_HEIGHT: usize = 20;

/// Count of all nodes in the merkle-tree
pub const MT_SIZE: usize = 2usize.pow(MT_HEIGHT as u32 + 1) - 1;

/// Index of the first commitment in the Merkle Tree
pub const MT_COMMITMENT_START: usize = 2usize.pow(MT_HEIGHT as u32) - 1;

/// Since before submitting a proof request the current root can change, we store the previous ones
const HISTORY_ARRAY_COUNT: usize = 100;

pub const STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT: usize = big_array_accounts_count(MT_SIZE, U256::SIZE);
const_assert_eq!(STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT, 7);

pub const DEFAULT_VALUES: [Fr; MT_HEIGHT + 1] = [
    Fr::new(BigInteger256::new([5691611937644566852, 17620820813181493991, 11578659137028816515, 2914824457222516369])),
    Fr::new(BigInteger256::new([1468893897005136691, 4638128118315565334, 13680790228231065622, 1299508272559288457])),
    Fr::new(BigInteger256::new([4698931903603764972, 15338172196201026259, 845461366581210430, 2490130590646071464])),
    Fr::new(BigInteger256::new([2920096157871139096, 13060020165704901188, 12943162699953729247, 2234479897611199809])),
    Fr::new(BigInteger256::new([15436988243663142679, 3814774368829636446, 13365591614313630306, 1576186299059485553])),
    Fr::new(BigInteger256::new([13328230243323890297, 11128679064958937924, 9357000335258360483, 2104290745598804102])),
    Fr::new(BigInteger256::new([3756085570044973516, 9536957579284113805, 2409437860359384358, 1444172494927843072])),
    Fr::new(BigInteger256::new([15545935430856831981, 6995399324906788259, 7975302095956667197, 278329136157021172])),
    Fr::new(BigInteger256::new([5490509226410727857, 11094896903217897359, 12299135231088992513, 424172748035087377])),
    Fr::new(BigInteger256::new([7568814144423326948, 7756902877083136468, 2643112947961160356, 2381594894185361981])),
    Fr::new(BigInteger256::new([11638450640257199038, 7595865483997155469, 13070293587080331272, 1693899333659075201])),
    Fr::new(BigInteger256::new([13017598453810020150, 5751567870812522253, 14128703922746499357, 1640897934100003677])),
    Fr::new(BigInteger256::new([16529257223091242829, 15078715808286425477, 14973738173337342358, 1311755140402558240])),
    Fr::new(BigInteger256::new([5817658827105621581, 11059894328710668944, 9229044364542587538, 2062834750464334044])),
    Fr::new(BigInteger256::new([17311028596387249094, 10845365150759738931, 17445412932997959742, 48033258554112463])),
    Fr::new(BigInteger256::new([16314769355260462117, 327129018270864341, 11408471150400527423, 2297888125986573548])),
    Fr::new(BigInteger256::new([3374525873845327982, 8842951786222469347, 14628822357859034252, 2329128191974466333])),
    Fr::new(BigInteger256::new([18120511978754371573, 206215742733117120, 9749843034257160914, 3297229344299070194])),
    Fr::new(BigInteger256::new([16524436797946508368, 5663459082060437627, 4786453218948112063, 1780966499111310984])),
    Fr::new(BigInteger256::new([3162363550698150530, 9486080942857866267, 15374008727889305678, 621823773387469172])),
    Fr::new(BigInteger256::new([0, 0, 0, 0])),
];

const ZERO: Fr = Fr::new(BigInteger256::new([0, 0, 0, 0]));

// The `StorageAccount` contains the active Merkle Tree that stores new commitments
// - the MT is stored as an array with the first element being the root and the second and third elements the layer below the root
// - in order to manage a growing number of commitments, once the MT is full it get's reset (and the root is stored elsewhere)
#[elusiv_account(pda_seed = b"storage", multi_account = (
    STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT;
    max_account_size(U256::SIZE)
))]
pub struct StorageAccount {
    bump_seed: u8,
    initialized: bool,

    pubkeys: [U256; STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT],
    finished_setup: bool,

    // Points to the next commitment in the active MT
    next_commitment_ptr: u64,

    // The amount of already finished MTs
    trees_count: u64,

    // The amount of archived MTs
    archived_count: u64,

    // Stores the last HISTORY_ARRAY_COUNT roots of the active tree
    active_mt_root_history: [U256; HISTORY_ARRAY_COUNT],
}

impl<'a, 'b, 't> BigArrayAccount<'t> for StorageAccount<'a, 'b, 't> {
    type T = U256;
    const VALUES_COUNT: usize = MT_SIZE;
}

impl<'a, 'b, 't> MultiInstanceAccount for StorageAccount<'a, 'b, 't> {
    const MAX_INSTANCES: u64 = 1;
}

impl<'a, 'b, 't> StorageAccount<'a, 'b, 't> {
    pub fn reset(&mut self) {
        self.set_next_commitment_ptr(&0);

        for i in 0..self.active_mt_root_history.len() {
            self.active_mt_root_history[i] = 0;
        }
    }

    /// Inserts commitment and the above hashes
    pub fn insert_commitment(&mut self, values: [U256; MT_HEIGHT + 1]) {
        let ptr = self.get_next_commitment_ptr() as usize;
        self.set_next_commitment_ptr(&(ptr as u64 + 1));

        // Save last root
        self.set_active_mt_root_history(ptr % HISTORY_ARRAY_COUNT, &self.get_root());

        // Insert values into the tree
        for (i, &value) in values.iter().enumerate() {
            let level = MT_HEIGHT - i;
            let index = ptr >> i;
            self.set_node(&value, index, level);
        }
    }

    /// `level`: 0 is the root level, `MT_HEIGHT` the commitment level
    pub fn get_node(&self, index: usize, level: usize) -> Fr {
        assert!(level <= MT_HEIGHT);

        let ptr = self.get_next_commitment_ptr() as usize;

        // Accessing a node, that is non-existent (yet) -> use const
        if (index >> level) >= (ptr >> level) {
            DEFAULT_VALUES[level]
        } else {
            u256_to_fr(&self.get(mt_array_index(index, level)))
        }
    }

    fn set_node(&mut self, value: &U256, index: usize, level: usize) {
        assert!(level <= MT_HEIGHT);

        self.set(mt_array_index(index, level), *value);
    }

    pub fn get_root(&self) -> U256 {
        fr_to_u256_le(&self.get_node(0, 0))
    }

    /// A root is valid if it's the current root or inside of the active_mt_root_history array
    pub fn is_root_valid(&self, root: U256) -> bool {
        root == self.get_root() || contains(root, self.active_mt_root_history)
    }

    pub fn get_mt_opening(&self, index: usize) -> [Fr; MT_HEIGHT] {
        let mut opening = [ZERO; MT_HEIGHT];
        let mut index = index;

        for i in 0..MT_HEIGHT {
            let level = MT_HEIGHT - i;
            let n_index = if index % 2 == 0 { index + 1 } else { index - 1};
            opening[i] = self.get_node(n_index, level);
            index = index >> 1;
        }

        opening
    }
}

pub fn mt_array_index(index: usize, level: usize) -> usize {
    2usize.pow(level as u32) - 1 + index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macros::{account, generate_storage_accounts, generate_storage_accounts_valid_size};
    use solana_program::{pubkey::Pubkey, account_info::AccountInfo};
    use super::super::program_account::{MultiAccountAccount, BigArrayAccount};
    use std::str::FromStr;

    macro_rules! storage_account {
        ($id: ident) => {
            let mut data = vec![0; StorageAccount::SIZE];
            generate_storage_accounts_valid_size!(accounts);
            let $id = StorageAccount::new(&mut data, &accounts[..]).unwrap();
        };
    }

    #[test]
    fn test_storage_account() {
        assert_eq!(STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT, 7);
    }

    #[test]
    fn test_get_default_mt_values() {
        storage_account!(storage_account);

        // Get default values for empty tree
        assert_eq!(storage_account.get_node(0, MT_HEIGHT), Fr::from_str("0").unwrap());
        assert_eq!(storage_account.get_node(0, 0), Fr::from_str("15019797232609675441998260052101280400536945603062888308240081994073687793470").unwrap());
        assert_eq!(storage_account.get_node(0, 1), Fr::from_str("10941962436777715901943463195175331263348098796018438960955633645115732864202").unwrap());
        assert_eq!(storage_account.get_node(0, MT_HEIGHT - 1), Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap());
        assert_eq!(storage_account.get_node(0, MT_HEIGHT - 2), Fr::from_str("7423237065226347324353380772367382631490014989348495481811164164159255474657").unwrap());
    }

    /*    // Insert commitments
        for i in 0..9 {
            storage_account.set_node(
                &fr_to_u256_le(&Fr::from_str(&format!("{}", i + 1)).unwrap()),
                i,
                MT_HEIGHT
            );
        }

        // Get commitments
        for i in 0..9 {
            assert_eq!(
                storage_account.get_node(i, MT_HEIGHT),
                Fr::from_str(&format!("{}", i + 1)).unwrap()
            );
        }
    }*/

    //
    //Fp256 "(0000000000000000000000000000000000000000000000000000000000000000)"
    //Fp256 "(2098F5FB9E239EAB3CEAC3F27B81E481DC3124D55FFED523A839EE8446B64864)" 14744269619966411208579211824598458697587494354926760081771325075741142829156
    //Fp256 "(1069673DCDB12263DF301A6FF584A7EC261A44CB9DC68DF067A4774460B1F1E1)" 7423237065226347324353380772367382631490014989348495481811164164159255474657
    //Fp256 "(18F43331537EE2AF2E3D758D50F72106467C6EEA50371DD528D57EB2B856D238)" 11286972368698509976183087595462810875513684078608517520839298933882497716792
    //Fp256 "(07F9D837CB17B0D36320FFE93BA52345F1B728571A568265CAAC97559DBC952A)" 3607627140608796879659380071776844901612302623152076817094415224584923813162
    //Fp256 "(2B94CF5E8746B3F5C9631F4C5DF32907A699C58C94B2AD4D7B5CEC1639183F55)" 19712377064642672829441595136074946683621277828620209496774504837737984048981
    //Fp256 "(2DEE93C5A666459646EA7D22CCA9E1BCFED71E6951B953611D11DDA32EA09D78)" 20775607673010627194014556968476266066927294572720319469184847051418138353016
    //Fp256 "(078295E5A22B84E982CF601EB639597B8B0515A88CB5AC7FA8A4AABE3C87349D)" 3396914609616007258851405644437304192397291162432396347162513310381425243293
    //Fp256 "(2FA5E5F18F6027A6501BEC864564472A616B2E274A41211A444CBE3A99F3CC61)" 21551820661461729022865262380882070649935529853313286572328683688269863701601
    //Fp256 "(0E884376D0D8FD21ECB780389E941F66E45E7ACCE3E228AB3E2156A614FCD747)" 6573136701248752079028194407151022595060682063033565181951145966236778420039
    //Fp256 "(1B7201DA72494F1E28717AD1A52EB469F95892F957713533DE6175E5DA190AF2)" 12413880268183407374852357075976609371175688755676981206018884971008854919922
    //Fp256 "(1F8D8822725E36385200C0B201249819A6E6E1E4650808B5BEBC6BFACE7D7636)" 14271763308400718165336499097156975241954733520325982997864342600795471836726
    //Fp256 "(2C5D82F66C914BAFB9701589BA8CFCFB6162B0A12ACF88A8D0879A0471B5F85A)" 20066985985293572387227381049700832219069292839614107140851619262827735677018
    //Fp256 "(14C54148A0940BB820957F5ADF3FA1134EF5C4AAA113F4646458F270E0BFBFD0)" 9394776414966240069580838672673694685292165040808226440647796406499139370960
    //Fp256 "(190D33B12F986F961E10C0EE44D8B9AF11BE25588CAD89D416118E4BF4EBE80C)" 11331146992410411304059858900317123658895005918277453009197229807340014528524
    //Fp256 "(22F98AA9CE704152AC17354914AD73ED1167AE6596AF510AA5B3649325E06C92)" 15819538789928229930262697811477882737253464456578333862691129291651619515538
    //Fp256 "(2A7C7C9B6CE5880B9F6F228D72BF6A575A526F29C66ECCEEF8B753D38BBA7323)" 19217088683336594659449020493828377907203207941212636669271704950158751593251
    //Fp256 "(2E8186E558698EC1C67AF9C14D463FFC470043C9C2988B954D75DD643F36B992)" 21035245323335827719745544373081896983162834604456827698288649288827293579666
    //Fp256 "(0F57C5571E9A4EAB49E2C8CF050DAE948AEF6EAD647392273546249D1C1FF10F)" 6939770416153240137322503476966641397417391950902474480970945462551409848591
    //Fp256 "(1830EE67B5FB554AD5F63D4388800E1CFE78E310697D46E43C9CE36134F72CCA)" 10941962436777715901943463195175331263348098796018438960955633645115732864202
    //Fp256 "(2134E76AC5D21AAB186C2BE1DD8F84EE880A1E46EAF712F9D371B6DF22191F3E)" 15019797232609675441998260052101280400536945603062888308240081994073687793470
    //Fp256 "(19DF90EC844EBC4FFEEBD866F33859B0C051D8C958EE3AA88F8F8DF3DB91A5B1)" 11702828337982203149177882813338547876343922920234831094975924378932809409969
}