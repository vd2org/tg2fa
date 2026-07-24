use maxminddb::{path, Mmap, Reader};
use std::net::IpAddr;
use std::path::PathBuf;
use log::info;

pub struct Geoip {
    city_db: Reader<Mmap>,
    asn_db: Reader<Mmap>,
}

#[derive(Clone, Debug)]
pub struct GeoipResult {
    pub country_code: String,
    pub country: String,
    pub city: String,
    pub org: String,
}

impl Default for GeoipResult {
    fn default() -> Self {
        GeoipResult { country_code: "".into(), country: NA.into(), city: NA.into(), org: NA.into() }
    }
}

const NA: &str = "NA";

impl Geoip {
    pub fn new(city_db_path: &PathBuf, asn_db_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        //SAFETY: The maxminddb crate uses unsafe code to memory-map the database files.
        // This usage is safe as long as the database files are not modified while being accessed.
        let city_db = unsafe { Reader::open_mmap(city_db_path) }?;
        let asn_db = unsafe { Reader::open_mmap(asn_db_path) }?;
        info!("Loaded GeoIP databases: city_db={} asn_db={}", city_db_path.display(), asn_db_path.display());
        Ok(Geoip { city_db, asn_db })
    }

    pub fn lookup(&self, ip: &str) -> Result<GeoipResult, Box<dyn std::error::Error>> {
        let ip: IpAddr = ip.parse()?;

        let city = self.city_db.lookup(ip)?;
        let asn = self.asn_db.lookup(ip)?;

        Ok(GeoipResult {
            country_code: city.decode_path(&path!["country", "iso_code"])?.unwrap_or("").into(),
            country: city.decode_path(&path!["country", "names", "en"])?.unwrap_or(NA).into(),
            city: city.decode_path(&path!["city", "names", "en"])?.unwrap_or(NA).into(),
            org: asn.decode_path(&path!["autonomous_system_organization"])?.unwrap_or(NA).into(),
        })
    }

    pub fn lookup_ok(&self, ip: &str) -> GeoipResult {
        self.lookup(ip).unwrap_or_default()
    }
}
