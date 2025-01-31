use many::types::events::{EventId, EventLog};
use many::types::ledger::TokenAmount;
use many::types::{CborRange, SortOrder};
use many::Identity;
use many_ledger::storage::LedgerStorage;
use std::collections::BTreeMap;
use std::ops::Bound;

fn setup() -> LedgerStorage {
    let symbol0 = Identity::anonymous();
    let id0 = Identity::public_key_raw([0; 28]);
    let id1 = Identity::public_key_raw([1; 28]);
    let id2 = Identity::public_key_raw([2; 28]);

    let symbols = BTreeMap::from_iter(vec![(symbol0, "MFX".to_string())].into_iter());
    let balances = BTreeMap::from([(id0, BTreeMap::from([(symbol0, TokenAmount::from(1000u16))]))]);
    let persistent_path = tempfile::tempdir().unwrap();

    let mut storage = many_ledger::storage::LedgerStorage::new(
        symbols,
        balances,
        persistent_path,
        id2,
        false,
        None,
        None,
    )
    .unwrap();

    for _ in 0..5 {
        storage
            .send(&id0, &id1, &symbol0, TokenAmount::from(100u16))
            .unwrap();
    }

    // Check that we have 5 events (5 sends).
    assert_eq!(storage.nb_events(), 5);

    storage
}

fn iter_asc(
    storage: &LedgerStorage,
    start: Bound<EventId>,
    end: Bound<EventId>,
) -> impl Iterator<Item = EventLog> + '_ {
    storage
        .iter(CborRange { start, end }, SortOrder::Ascending)
        .into_iter()
        .map(|(_, v)| minicbor::decode(&v).expect("Iterator item not an event."))
}

#[test]
fn range_works() {
    let storage = setup();

    // Get the first event ID.
    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Unbounded);
    let first_ev = iter.next().expect("No events?");
    let first_id = first_ev.id;
    let last_ev = iter.last().expect("Only 1 event");
    let last_id = last_ev.id;

    // Make sure exclusive range removes the first_id.
    assert!(iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded
    )
    .all(|x| x.id != first_id));

    let iter = iter_asc(
        &storage,
        Bound::Excluded(first_id.clone()),
        Bound::Unbounded,
    );
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);

    // Make sure exclusive range removes the last_id.
    assert!(
        iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()))
            .all(|x| x.id != last_id)
    );

    let mut iter = iter_asc(&storage, Bound::Unbounded, Bound::Excluded(last_id.clone()));
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);

    // Make sure inclusive bounds include first_id.
    let mut iter = iter_asc(
        &storage,
        Bound::Included(first_id.clone()),
        Bound::Included(last_id.clone()),
    );
    assert_eq!(iter.next().expect("Should have a first item").id, first_id);
    assert_eq!(iter.last().expect("Should have a last item").id, last_id);
}
