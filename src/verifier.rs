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
                    BigInteger256([964940542692076337, 7429335272291332545, 16053119850818242642, 209668931307008146]).into(),
                    BigInteger256([15048840897342206881, 1775098383405223162, 11542831195503446111, 2691074708033833167]).into(),
                    BigInteger256([1, 0, 0, 0]).into(),
                )
            ),
        beta_g2:
            G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        BigInteger256([3020434666745593713, 3611609683192261384, 1882634532092213981, 575673890746274363]).into(),
                        BigInteger256([14226971224081794786, 10477292665640905465, 9855479593556991735, 2338906218810058812]).into()
                    ),
                    Fq2::new(
                        BigInteger256([18372933448630018139, 17264320071482471380, 263667372686397175, 2797472776149267043]).into(),
                        BigInteger256([7183319866686709425, 3603251436185228292, 17333626804559607732, 2571807521409146037]).into()
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
                        BigInteger256([16670387869202145283, 2539093323243198355, 4813955905471484973, 2834119673392771789]).into(),
                        BigInteger256([12417138311461343224, 1156601185734773895, 4325852741385973885, 1064643991971571586]).into()
                    ),
                    Fq2::new(
                        BigInteger256([17720809885491763331, 7519416002493525461, 154969853702597820, 1103344961009327595]).into(),
                        BigInteger256([11792272639851030683, 5832973897530426788, 11107456413962269029, 2752240891350838437]).into()
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
						BigInteger256([18130279453774375296, 14018417424240261822, 14795651430753089745, 520219591205570379]).into(),
						BigInteger256([4185042791627673567, 14495191540120992234, 14781629527392888996, 2684356151855859559]).into(),
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
            String::from("1316113212563891598888754501606081739259024890435162057050888902691376376625"),
            String::from("16892149719854379438845763282041361577247645787479120562253203736768302805921"),
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
                String::from("3613563578620241269208037298822152848934114508965518333097473378003542441841"),
                String::from("14681552284999319850840636316291698366343878565262564524371684131513732531938"),
            ],
            [
                String::from("17560021217863559684574565902752491450419133430853933825093363277388115091547"),
                String::from("16143497455717868736436518701065041137845097001653287781689667003777816600241"),
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
                String::from("3613563578620241269208037298822152848934114508965518333097473378003542441841"),
                String::from("14681552284999319850840636316291698366343878565262564524371684131513732531938"),
            ],
            [
                String::from("17560021217863559684574565902752491450419133430853933825093363277388115091547"),
                String::from("16143497455717868736436518701065041137845097001653287781689667003777816600241"),
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
                String::from("3613563578620241269208037298822152848934114508965518333097473378003542441841"),
                String::from("14681552284999319850840636316291698366343878565262564524371684131513732531938"),
            ],
            [
                String::from("17560021217863559684574565902752491450419133430853933825093363277388115091547"),
                String::from("16143497455717868736436518701065041137845097001653287781689667003777816600241"),
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
				String::from("3265471298738635481609918930070781891617155948546681492275809777373453805952"),
				String::from("16849976659210328399952086867668208642427550020443220678873780756028399734751"),
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