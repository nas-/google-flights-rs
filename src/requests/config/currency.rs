use std::fmt;

use clap::ValueEnum;

/// Result currency for flight search.
#[derive(Debug, Clone, Default, ValueEnum)]
pub enum Currency {
    AlbanianLek,
    AlgerianDinar,
    ArgentinePeso,
    ArmenianDram,
    ArubanFlorin,
    AustralianDollar,
    AzerbaijaniManat,
    BahamianDollar,
    BahrainiDinar,
    BelarusianRouble,
    BermudianDollar,
    BosniaHerzegovinaMark,
    BrazilianReal,
    BritishPound,
    BulgarianLev,
    CanadianDollar,
    CFPFranc,
    ChileanPeso,
    ChineseYuan,
    ColombianPeso,
    CostaRicanColon,
    CubanPeso,
    CzechKoruna,
    DanishKrone,
    DominicanPeso,
    EgyptianPound,
    #[default]
    Euro,
    GeorgianLari,
    HongKongDollar,
    HungarianForint,
    IcelandicKrona,
    IndianRupee,
    IndonesianRupiah,
    IranianRial,
    IsraeliNewShekel,
    JamaicanDollar,
    JapaneseYen,
    JordanianDinar,
    KazakhstaniTenge,
    KuwaitiDinar,
    LebanesePound,
    MacedonianDenar,
    MalaysianRinggit,
    MexicanPeso,
    MoldovanLeu,
    MoroccanDirham,
    NewTaiwanDollar,
    NewZealandDollar,
    NorwegianKrone,
    OmaniRial,
    PakistaniRupee,
    PanamanianBalboa,
    PeruvianSol,
    PhilippinePeso,
    PolishZloty,
    QatariRiyal,
    RomanianLeu,
    RussianRouble,
    SaudiRiyal,
    SerbianDinar,
    SingaporeDollar,
    SouthAfricanRand,
    SouthKoreanWon,
    SwedishKrona,
    SwissFranc,
    ThaiBaht,
    TurkishLira,
    UkrainianHryvnia,
    UnitedArabEmiratesDirham,
    USDollar,
    VietnameseDong,
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let res = match self {
            Currency::AlbanianLek => "ALL",
            Currency::AlgerianDinar => "DZD",
            Currency::ArgentinePeso => "ARS",
            Currency::ArmenianDram => "AMD",
            Currency::ArubanFlorin => "AWG",
            Currency::AustralianDollar => "AUD",
            Currency::AzerbaijaniManat => "AZN",
            Currency::BahamianDollar => "BSD",
            Currency::BahrainiDinar => "BHD",
            Currency::BelarusianRouble => "BYN",
            Currency::BermudianDollar => "BMD",
            Currency::BosniaHerzegovinaMark => "BAM",
            Currency::BrazilianReal => "BRL",
            Currency::BritishPound => "GBP",
            Currency::BulgarianLev => "BGN",
            Currency::CanadianDollar => "CAD",
            Currency::CFPFranc => "XPF",
            Currency::ChileanPeso => "CLP",
            Currency::ChineseYuan => "CNY",
            Currency::ColombianPeso => "COP",
            Currency::CostaRicanColon => "CRC",
            Currency::CubanPeso => "CUP",
            Currency::CzechKoruna => "CZK",
            Currency::DanishKrone => "DKK",
            Currency::DominicanPeso => "DOP",
            Currency::EgyptianPound => "EGP",
            Currency::Euro => "EUR",
            Currency::GeorgianLari => "GEL",
            Currency::HongKongDollar => "HKD",
            Currency::HungarianForint => "HUF",
            Currency::IcelandicKrona => "ISK",
            Currency::IndianRupee => "INR",
            Currency::IndonesianRupiah => "IDR",
            Currency::IranianRial => "IRR",
            Currency::IsraeliNewShekel => "ILS",
            Currency::JamaicanDollar => "JMD",
            Currency::JapaneseYen => "JPY",
            Currency::JordanianDinar => "JOD",
            Currency::KazakhstaniTenge => "KZT",
            Currency::KuwaitiDinar => "KWD",
            Currency::LebanesePound => "LBP",
            Currency::MacedonianDenar => "MKD",
            Currency::MalaysianRinggit => "MYR",
            Currency::MexicanPeso => "MXN",
            Currency::MoldovanLeu => "MDL",
            Currency::MoroccanDirham => "MAD",
            Currency::NewTaiwanDollar => "TWD",
            Currency::NewZealandDollar => "NZD",
            Currency::NorwegianKrone => "NOK",
            Currency::OmaniRial => "OMR",
            Currency::PakistaniRupee => "PKR",
            Currency::PanamanianBalboa => "PAB",
            Currency::PeruvianSol => "PEN",
            Currency::PhilippinePeso => "PHP",
            Currency::PolishZloty => "PLN",
            Currency::QatariRiyal => "QAR",
            Currency::RomanianLeu => "RON",
            Currency::RussianRouble => "RUB",
            Currency::SaudiRiyal => "SAR",
            Currency::SerbianDinar => "RSD",
            Currency::SingaporeDollar => "SGD",
            Currency::SouthAfricanRand => "ZAR",
            Currency::SouthKoreanWon => "KRW",
            Currency::SwedishKrona => "SEK",
            Currency::SwissFranc => "CHF",
            Currency::ThaiBaht => "THB",
            Currency::TurkishLira => "TRY",
            Currency::UkrainianHryvnia => "UAH",
            Currency::UnitedArabEmiratesDirham => "AED",
            Currency::USDollar => "USD",
            Currency::VietnameseDong => "VND",
        };
        f.write_str(res)
    }
}

impl Currency {
    /// Parse a currency from its ISO-4217 code (case-insensitive), e.g. `"USD"`.
    ///
    /// This is the reverse of [`Display`](std::fmt::Display): every variant
    /// renders to its ISO code, so we match the input against each variant's
    /// rendered code.
    pub fn from_code(code: &str) -> Option<Self> {
        let up = code.trim().to_uppercase();
        <Self as ValueEnum>::value_variants()
            .iter()
            .find(|c| c.to_string() == up)
            .cloned()
    }
}

impl std::str::FromStr for Currency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_code(s).ok_or_else(|| {
            format!("unknown currency code {s:?} (expected ISO-4217, e.g. USD, EUR, GBP)")
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn currency_display_spot_check() {
        assert_eq!(Currency::Euro.to_string(), "EUR");
        assert_eq!(Currency::USDollar.to_string(), "USD");
        assert_eq!(Currency::BritishPound.to_string(), "GBP");
        assert_eq!(Currency::JapaneseYen.to_string(), "JPY");
        assert_eq!(Currency::SwissFranc.to_string(), "CHF");
        assert_eq!(Currency::AustralianDollar.to_string(), "AUD");
    }

    #[test]
    fn from_code_parses_iso_case_insensitive() {
        assert!(matches!(
            Currency::from_code("USD"),
            Some(Currency::USDollar)
        ));
        assert!(matches!(
            Currency::from_code("usd"),
            Some(Currency::USDollar)
        ));
        assert!(matches!(Currency::from_code(" eur "), Some(Currency::Euro)));
        assert!(matches!(
            Currency::from_code("GBP"),
            Some(Currency::BritishPound)
        ));
        assert!(Currency::from_code("us-dollar").is_none());
        assert!(Currency::from_code("ZZZ").is_none());
    }

    #[test]
    fn from_str_errors_on_unknown() {
        assert!(<Currency as FromStr>::from_str("USD").is_ok());
        let err = <Currency as FromStr>::from_str("ZZZ").unwrap_err();
        assert!(err.contains("unknown currency"));
    }
}
