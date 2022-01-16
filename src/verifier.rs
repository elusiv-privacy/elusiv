//! **********
//!
//! Generated using circom_verifier_key 0.1.0
//!
//! **********

use ark_bn254::{ Bn254, Fq2, G1Affine, G2Affine, G2Projective, G1Projective };
use ark_ff::{ BigInteger256 };
use ark_groth16::VerifyingKey;

pub fn verification_key() -> VerifyingKey<Bn254> {
    VerifyingKey {
        alpha_g1:
            G1Affine::from(
                G1Projective::new(
                    BigInteger256([4442864439166756984, 4574045506909349437, 10701839041301083415, 1612794170237378160]).into(),
                    BigInteger256([2454593247855632740, 17197849827163444358, 3273120395094234488, 3314060189894239153]).into(),
                    BigInteger256([1, 0, 0, 0]).into(),
                )
            ),
        beta_g2:
            G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        BigInteger256([16398347503496443439, 13963903538166369317, 3988987101915011817, 3397216676773544885]).into(),
                        BigInteger256([5290166403029168784, 3278426767036501167, 5065361473200159128, 1408948115050541142]).into()
                    ),
                    Fq2::new(
                        BigInteger256([14313305366676052395, 9198219814380103355, 17323607755254864615, 1428047260232083657]).into(),
                        BigInteger256([9853284749695102720, 14559315917683145672, 7670086236784222727, 2994230725432155362]).into()
                    ),
                    Fq2::new(
                        BigInteger256([1, 0, 0, 0]).into(),
                        BigInteger256([0, 0, 0, 0]).into()
                    ),
                )
            ),
        gamma_g2:
            G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        BigInteger256([5106727233969649389, 7440829307424791261, 4785637993704342649, 1729627375292849782]).into(),
                        BigInteger256([10945020018377822914, 17413811393473931026, 8241798111626485029, 1841571559660931130]).into()
                    ),
                    Fq2::new(
                        BigInteger256([5541340697920699818, 16416156555105522555, 5380518976772849807, 1353435754470862315]).into(),
                        BigInteger256([6173549831154472795, 13567992399387660019, 17050234209342075797, 650358724130500725]).into()
                    ),
                    Fq2::new(
                        BigInteger256([1, 0, 0, 0]).into(),
                        BigInteger256([0, 0, 0, 0]).into()
                    ),
                )
            ),
        delta_g2:
            G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        BigInteger256([11329895644345657761, 8544584578083840128, 4480016363514441694, 1342298006971139878]).into(),
                        BigInteger256([6736558649910569265, 16825251804905863558, 1771647314935346749, 1457449162406174619]).into()
                    ),
                    Fq2::new(
                        BigInteger256([8909432121399871928, 13957644023099575633, 11350961627972833401, 1674543466805214715]).into(),
                        BigInteger256([3711857448074524946, 11140758393664745906, 17839848644685055599, 3016742244934197552]).into()
                    ),
                    Fq2::new(
                        BigInteger256([1, 0, 0, 0]).into(),
                        BigInteger256([0, 0, 0, 0]).into()
                    ),
                )
            ),
        gamma_abc_g1:
            vec![
				G1Affine::from(
					G1Projective::new(
						BigInteger256([13739637505665344557, 12140466552464713478, 4823807000201120938, 485344879676783155]).into(),
						BigInteger256([5164223161147448208, 6105401401746479682, 10044962603448347745, 1663429704226060982]).into(),
						BigInteger256([1, 0, 0, 0]).into(),
					)
				),				G1Affine::from(
					G1Projective::new(
						BigInteger256([16218378012903560709, 8146959098842928444, 15763124201027407965, 2205856214193053583]).into(),
						BigInteger256([3554772135543592235, 11348595406388699585, 16437605627876144447, 1528340656053631929]).into(),
						BigInteger256([1, 0, 0, 0]).into(),
					)
				),				G1Affine::from(
					G1Projective::new(
						BigInteger256([3722283020989762675, 7263273322679665080, 18078280640854005980, 788904760196518231]).into(),
						BigInteger256([2536013134077706826, 14574065942500886312, 11121509841367687982, 3304416991182840720]).into(),
						BigInteger256([1, 0, 0, 0]).into(),
					)
				),
			]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ark_bn254::{ Fq };
    use num_bigint::BigUint;
    use std::convert::TryFrom;
    use std::str::FromStr;

    fn p_to_g1(p: &[String; 3]) -> G1Affine {
        G1Affine::from(G1Projective::new(
            str_to_fq(&p[0]),
            str_to_fq(&p[1]),
            str_to_fq(&p[2]),
        ))
    }

    fn p_to_g2(p: [[String; 2]; 3]) -> G2Affine {
        G2Affine::from(G2Projective::new(
            str_to_fq2(&p[0][0], &p[0][1]),
            str_to_fq2(&p[1][0], &p[1][1]),
            str_to_fq2(&p[2][0], &p[2][1]),
        ))
    }

    fn str_to_fq(str: &str) -> Fq {
        BigInteger256::try_from(BigUint::from_str(str).unwrap())
            .unwrap()
            .into()
    }

    fn str_to_fq2(str0: &str, str1: &str) -> Fq2 {
        Fq2::new(str_to_fq(str0), str_to_fq(str1))
    }

    #[test]
    fn test_alpha() {
        let alpha_src = [
            String::from("10123673084818568295286052306704560916455280017700734331063512290856062150776"),
            String::from("20802692969161041380541101349833572686334107756741364135938182902602319126884"),
            String::from("1"),
        ];

        assert_eq!(
            p_to_g1(&alpha_src),
            verification_key().alpha_g1
        )
    }

    #[test]
    fn test_beta() {
        let beta_src = [
            [
                String::from("21324674697259791140581733928795308652764605736388463578408383302023588722223"),
                String::from("8844110658053544550193473526169753900164999384738237283921910357168390042256"),
            ],
            [
                String::from("8963997935417007237294911607079302765467115895852003893298059777604552694187"),
                String::from("18795090882758302474220443880212963900205131094323732218686397999130227617536"),
            ],
            [
                String::from("1"),
                String::from("0"),
            ],
        ];

        assert_eq!(
            p_to_g2(beta_src),
            verification_key().beta_g2
        )
    }

    #[test]
    fn test_gamma() {
        let gamma_src = [
            [
                String::from("21324674697259791140581733928795308652764605736388463578408383302023588722223"),
                String::from("8844110658053544550193473526169753900164999384738237283921910357168390042256"),
            ],
            [
                String::from("8963997935417007237294911607079302765467115895852003893298059777604552694187"),
                String::from("18795090882758302474220443880212963900205131094323732218686397999130227617536"),
            ],
            [
                String::from("1"),
                String::from("0"),
            ],
        ];

        assert_eq!(
            p_to_g2(gamma_src),
            verification_key().beta_g2
        )
    }

    #[test]
    fn test_delta() {
        let delta_src = [
            [
                String::from("21324674697259791140581733928795308652764605736388463578408383302023588722223"),
                String::from("8844110658053544550193473526169753900164999384738237283921910357168390042256"),
            ],
            [
                String::from("8963997935417007237294911607079302765467115895852003893298059777604552694187"),
                String::from("18795090882758302474220443880212963900205131094323732218686397999130227617536"),
            ],
            [
                String::from("1"),
                String::from("0"),
            ],
        ];

        assert_eq!(
            p_to_g2(delta_src),
            verification_key().beta_g2
        )
    }

    #[test]
    fn test_gamma_abc() {
        let gamma_src = vec![
			[
				String::from("3046559186480175311907938209401029432017498122078078606207096246568213357613"),
				String::from("10441517483091160494363401958379603875962190532974887769651623795215963904912"),
				String::from("1"),
			],
			[
				String::from("13846383870124710441977272399908116142586479432090723842764990196810447279621"),
				String::from("9593549784376271172454834083181406598164909114000645465764656114950702912811"),
				String::from("1"),
			],
			[
				String::from("4952035439284377830187430865327702027126861700812079882385934496623439645811"),
				String::from("20742161629795043675707644785770496054171900884201918071458038004110949433930"),
				String::from("1"),
			],
		];
        let ic = verification_key().gamma_abc_g1;

        for i in 0..ic.len() {
            assert_eq!(
                p_to_g1(&gamma_src[i]),
                ic[i]
            )
        }
    }

    #[test]
    fn test_invalid() {
        let invalid_src = [
            [
                String::from("0"),
                String::from("0"),
            ],
            [
                String::from("0"),
                String::from("0"),
            ],
            [
                String::from("0"),
                String::from("0"),
            ],
        ];

        assert_ne!(
            p_to_g2(invalid_src),
            verification_key().beta_g2
        )
    }
}