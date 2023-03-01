#![allow(missing_docs)]
use core::hash::Hash;
use std::{
    collections::{HashMap, HashSet},
    sync::MutexGuard,
};

use args::{FindArguments, UpdateArguments};
use errors::{locking, Cardinality, Conflict, KeyNotFound};
use http_problem::Result;
use identifier::{Identifier, Sequence};
pub mod args;
pub mod errors;
pub mod identifier;

use std::sync::Mutex;

#[derive(Debug)]
pub struct FakeDb<K, V, I>
where
    K: Eq + Hash + std::fmt::Debug + Clone,
    V: Clone,
    I: Identifier<V, Id = K>,
{
    storage: Mutex<HashMap<K, V>>,
    identifier: I,
}

impl<V> Default for FakeDb<u32, V, Sequence>
where
    V: Clone,
{
    fn default() -> Self {
        Self::new(Sequence::new())
    }
}

impl<K, V, I> FakeDb<K, V, I>
where
    K: Eq + Hash + std::fmt::Debug + Clone,
    V: Clone,
    I: Identifier<V, Id = K>,
{
    pub fn new(identifier: I) -> Self {
        Self {
            storage: Mutex::new(HashMap::new()),
            identifier,
        }
    }

    /// # Errors
    /// Locking may result in a error
    pub fn find_by_id(&self, id: &K) -> Result<Option<V>> {
        let storage = self.storage.lock().map_err(locking)?;
        Ok(storage.get(id).cloned())
    }

    /// # Errors
    /// Locking may result in a error
    pub fn find_one(&self, args: FindArguments<V>) -> Result<Option<V>> {
        self.find_many(args).map(|v| v.first().cloned())
    }

    /// # Errors
    /// Locking may result in a error
    pub fn find_many(&self, args: FindArguments<V>) -> Result<Vec<V>> {
        let storage = self.storage.lock().map_err(locking)?;
        Ok(Self::_find_many(&storage, args))
    }

    fn _find_many(
        storage: &MutexGuard<'_, HashMap<K, V>>,
        FindArguments { matcher, order }: FindArguments<V>,
    ) -> Vec<V> {
        let mut matches: Vec<V> = storage.values().filter(matcher).cloned().collect();
        if let Some(order) = order {
            matches.sort_by(order);
        }

        matches
    }

    /// # Errors
    ///  * Inserting a value with a in already insert results in a Conflict
    ///    error
    ///  * Locking may result in a error
    pub fn insert(&self, value: V) -> Result<()> {
        let id = self.identifier.new_id(&value);
        let mut storage = self.storage.lock().map_err(locking)?;
        if storage.get(&id).is_some() {
            Err(Conflict {
                key: format!("{id:?}"),
            }
            .into())
        } else {
            storage.insert(id, value);
            Ok(())
        }
    }

    /// # Errors
    ///  * Inserting a value with a in already insert results in a Conflict
    ///    error
    ///  * Inserting values with the same id results in a Cardinality error
    ///  * Locking may result in a error
    pub fn insert_many(&self, values: Vec<V>) -> Result<()> {
        self.check_cardinality(&values)?;
        let storage = self.storage.lock().map_err(locking)?;

        self._insert_many(storage, values)
    }

    fn _insert_many(
        &self,
        mut storage: MutexGuard<'_, HashMap<K, V>>,
        values: Vec<V>,
    ) -> Result<()> {
        let mut stage_storage = HashMap::<K, V>::with_capacity(values.len());
        for value in values {
            let id = self.identifier.new_id(&value);
            if storage.get(&id).is_none() {
                stage_storage.insert(id, value);
            } else {
                return Err(Conflict {
                    key: format!("{id:?}"),
                }
                .into());
            }
        }
        storage.extend(stage_storage);

        Ok(())
    }

    /// # Errors
    ///  * Updating a value not in storage results in a KeyNotFound error
    ///  * Locking may result in a error
    pub fn update(&self, value: V) -> Result<()> {
        let id = self.identifier.new_id(&value);
        let id_err = id.clone();
        let mut storage = self.storage.lock().map_err(locking)?;
        storage
            .get(&id)
            .map(|_| ())
            .and_then(|_| storage.insert(id, value).map(|_| {}))
            .ok_or_else(|| {
                KeyNotFound {
                    key: format!("{id_err:?}"),
                }
                .into()
            })
    }

    /// # Errors
    ///  * Updating a values resulting in duplicated ids results in a Conflict
    /// error
    ///  * Locking may result in a error
    pub fn update_many(
        &self,
        UpdateArguments::<V> {
            mut matcher,
            mut updater,
        }: UpdateArguments<V>,
    ) -> Result<()> {
        let mut storage = self.storage.lock().map_err(locking)?;

        let values_before: Vec<_> = storage
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let ids: Vec<K> = storage
            .iter()
            .filter(|(_, v)| matcher(v))
            .map(|(id, _)| id)
            .cloned()
            .collect();

        let values: Vec<_> = ids
            .iter()
            .map(|id| {
                let value = storage.remove(id).expect("unreachable");
                (id.clone(), value)
            })
            .collect();

        let mut temp_storage = HashMap::<K, V>::new();
        for (id, mut value) in values {
            updater(&mut value);

            let id = if self.identifier.is_autogenerated() {
                self.identifier.new_id(&value)
            } else {
                id
            };

            if storage.get(&id).or_else(|| temp_storage.get(&id)).is_none() {
                temp_storage.insert(id, value);
            } else {
                storage.extend(values_before);

                return Err(Conflict {
                    key: format!("{id:?}"),
                }
                .into());
            }
        }
        storage.extend(temp_storage);

        Ok(())
    }

    /// # Errors
    /// Locking may result in a error
    pub fn delete_by_id(&self, id: &K) -> Result<Option<V>> {
        let mut storage = self.storage.lock().map_err(locking)?;
        Ok(storage.remove(id))
    }

    /// # Errors
    /// Locking may result in a error
    pub fn delete_many<M: FnMut(&&V) -> bool>(&self, mut matcher: M) -> Result<Vec<Option<V>>> {
        // No Option
        let mut storage = self.storage.lock().map_err(locking)?;

        let to_remove: Vec<_> = storage
            .iter()
            .filter(|(_, value)| matcher(value))
            .map(|(id, _)| id)
            .cloned()
            .collect();

        Ok(to_remove.iter().map(|id| storage.remove(id)).collect())
    }

    fn check_cardinality(&self, values: &[V]) -> Result<()> {
        let mut ids = HashSet::<K>::with_capacity(values.len());
        for value in values {
            let id = self.identifier.new_id(value);
            let key = format!("{id:?}");

            if !ids.insert(id) {
                return Err(Cardinality {
                    key: format!("{key:?}"),
                }
                .into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    pub struct Country {
        pub id: u32,
        pub name: &'static str,
    }

    struct CountryId();

    impl Identifier<Country> for CountryId {
        type Id = u32;

        fn new_id(&self, value: &Country) -> Self::Id {
            value.id
        }

        fn is_autogenerated(&self) -> bool {
            true
        }
    }

    #[test]
    pub fn test_db_reads_from_hash_map() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![(
                    378,
                    Country {
                        id: 378,
                        name: "San Marino",
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        let country = db
            .find_by_id(&378)
            .unwrap()
            .expect("san marino wasn't found");

        assert_eq!(country.id, 378);
        assert_eq!(country.name, "San Marino");
    }

    #[test]
    pub fn test_db_fails_to_read_from_hash_map() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![(
                    378,
                    Country {
                        id: 378,
                        name: "San Marino",
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        let error = db.find_by_id(&1).unwrap();

        assert!(error.is_none());
    }

    #[test]
    pub fn test_db_writes_one_to_storage() {
        let db = FakeDb::new(CountryId());

        db.insert(Country {
            id: 7,
            name: "Kazakhstan",
        })
        .expect("db did not write Kazakhstan");

        let country = db.find_by_id(&7).unwrap().expect("kazakhstan wasn't found");
        assert_eq!(country.id, 7);
        assert_eq!(country.name, "Kazakhstan");
    }

    #[test]
    pub fn test_db_writes_many_to_storage() {
        let db = FakeDb::new(CountryId());

        db.insert_many(vec![
            Country {
                id: 49,
                name: "Germany",
            },
            Country {
                id: 39,
                name: "Italy",
            },
        ])
        .expect("db failed to insert_many");

        let countries = db
            .find_many(args!(FindArguments<Country> {
                matcher: |_| true,
                order: |c1, c2| c1.id.cmp(&c2.id),
            }))
            .unwrap();

        assert_eq!(countries.len(), 2);
        assert_eq!(countries[0].id, 39);
        assert_eq!(countries[1].id, 49);
    }

    #[test]
    pub fn test_db_fails_to_write_many_to_storage_when_cardinality_is_infringed() {
        let db = FakeDb::new(CountryId());

        db.insert_many(vec![
            Country {
                id: 852,
                name: "Hong Kong",
            },
            Country {
                id: 852,
                name: "Hong Kong",
            },
        ])
        .expect_err("cardinality infringed");

        let countries = db
            .find_many(args!(FindArguments<Country> {
                order: |c1, c2| c1.id.cmp(&c2.id),
            }))
            .unwrap();

        assert_eq!(countries.len(), 0);
    }

    #[test]
    pub fn test_db_fails_to_write_many_when_any_entry_exists() {
        let db = FakeDb::new(CountryId());

        let north_korea = Country {
            id: 850,
            name: "North Korea",
        };

        let south_korea = Country {
            id: 82,
            name: "South Korea",
        };

        db.insert(south_korea.clone())
            .expect("db did not write North Korea");

        db.insert_many(vec![north_korea, south_korea])
            .expect_err("db did not failed to write many to db");

        let countries = db.find_many(args!(FindArguments<Country> {})).unwrap();

        assert_eq!(countries.len(), 1);
    }

    #[test]
    pub fn test_db_fails_to_write_when_a_entry_exists() {
        let db = FakeDb::new(CountryId());

        let country = Country {
            id: 7,
            name: "Kazakhstan",
        };
        db.insert(country.clone())
            .expect("db did not write Kazakhstan");
        let result = db.insert(country);
        assert!(result.is_err());
    }

    #[test]
    pub fn test_db_updates_when_a_entry_exists() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![(
                    55,
                    Country {
                        id: 55,
                        name: "Uruguay",
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        let country = Country {
            id: 55,
            name: "Brazil",
        };
        db.update(country).expect("db did not update id 55");
        let brazil = db.find_by_id(&55).unwrap().expect("db did not find brazil");

        assert_eq!(brazil.id, 55);
        assert_eq!(brazil.name, "Brazil");
    }

    #[test]
    pub fn test_db_update_many_with_custom_matcher() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![
                    (
                        51,
                        Country {
                            id: 51,
                            name: "Peru",
                        },
                    ),
                    (
                        56,
                        Country {
                            id: 56,
                            name: "Chile",
                        },
                    ),
                    (
                        506,
                        Country {
                            id: 506,
                            name: "Costa Rica",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };

        let args = args!(UpdateArguments<Country>{
            matcher: |&country| country.id < 506,
            updater: |country| country.name = "Unknown",
        });
        db.update_many(args).expect("db did not update many");
        let peru = db.find_by_id(&51).unwrap().expect("db did not find id 51");
        let chile = db.find_by_id(&56).unwrap().expect("db did not find id 56");
        let costa_rica = db
            .find_by_id(&506)
            .unwrap()
            .expect("db did not find id 506");

        assert_eq!(peru.name, "Unknown");
        assert_eq!(chile.name, "Unknown");
        assert_eq!(costa_rica.name, "Costa Rica");
    }

    #[test]
    pub fn test_db_update_many_fails_when_id_is_duplicated() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![
                    (
                        51,
                        Country {
                            id: 51,
                            name: "Peru",
                        },
                    ),
                    (
                        56,
                        Country {
                            id: 56,
                            name: "Chile",
                        },
                    ),
                    (
                        506,
                        Country {
                            id: 506,
                            name: "Costa Rica",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };

        let args = args!(UpdateArguments<Country> {
            updater: |country| country.id = 51,
            matcher: |&country| country.id < 506,
        });
        db.update_many(args).expect_err("db did not update many");
        let peru = db.find_by_id(&51).unwrap().expect("db did not find id 51");
        let chile = db.find_by_id(&56).unwrap().expect("db did not find id 56");
        let costa_rica = db
            .find_by_id(&506)
            .unwrap()
            .expect("db did not find id 506");

        assert_eq!(peru.name, "Peru");
        assert_eq!(chile.name, "Chile");
        assert_eq!(costa_rica.name, "Costa Rica");
    }

    #[test]
    pub fn test_db_fails_to_update_when_a_entry_dont_exists() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![(
                    1,
                    Country {
                        id: 1,
                        name: "United States of America",
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        let country = Country {
            id: 55,
            name: "Brazil",
        };
        let error = db.update(country);

        assert!(error.is_err());
    }

    #[test]
    fn test_delete_many_deletes_all_matches() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![
                    (
                        243,
                        Country {
                            id: 243,
                            name: "Democratic Republic of the Congo",
                        },
                    ),
                    (
                        242,
                        Country {
                            id: 242,
                            name: "Republic of the Congo",
                        },
                    ),
                    (
                        250,
                        Country {
                            id: 250,
                            name: "Rwanda",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };

        db.delete_many(|country| country.id < 250).ok();

        assert!(db.find_by_id(&242).unwrap().is_none());
        assert!(db.find_by_id(&242).unwrap().is_none());
        assert!(db.find_by_id(&250).unwrap().is_some());
    }

    #[test]
    pub fn test_db_finds_by_custom_match() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![(
                    506,
                    Country {
                        id: 506,
                        name: "Costa Rica",
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        let args = args!(FindArguments<Country> {
            matcher: |country: &&Country| country.name == "Costa Rica",
        });
        let costa_rica = db
            .find_one(args)
            .expect("db did not find a country called 'Costa Rica'")
            .unwrap();

        assert_eq!(costa_rica.id, 506);
        assert_eq!(costa_rica.name, "Costa Rica");
    }

    #[test]
    pub fn test_db_finds_many_by_custom_match() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![
                    (
                        11,
                        Country {
                            id: 11,
                            name: "Armenia",
                        },
                    ),
                    (
                        10,
                        Country {
                            id: 10,
                            name: "Argentina",
                        },
                    ),
                    (
                        9,
                        Country {
                            id: 9,
                            name: "Afghanistan",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };

        let args = args!(FindArguments<Country> {
            order: |c1: &Country, c2: &Country| c1.id.cmp(&c2.id),
            matcher: |country| country.id < 11,
        });
        let countries = db.find_many(args).unwrap();

        assert_eq!(countries.len(), 2);
        assert_eq!(countries[0].id, 9);
        assert_eq!(countries[0].name, "Afghanistan");
        assert_eq!(countries[1].id, 10);
        assert_eq!(countries[1].name, "Argentina");
    }

    #[test]
    pub fn test_db_deletes_correct_entry() {
        let db = FakeDb {
            identifier: CountryId(),
            storage: Mutex::new(
                vec![
                    (
                        30,
                        Country {
                            id: 30,
                            name: "Greece",
                        },
                    ),
                    (
                        90,
                        Country {
                            id: 90,
                            name: "Turkey",
                        },
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };

        let turkey = db
            .delete_by_id(&90)
            .unwrap()
            .expect("db did not deleted Turkey");
        let error = db.find_by_id(&90).unwrap();

        assert!(error.is_none());
        assert_eq!(turkey.id, 90);
    }
}
