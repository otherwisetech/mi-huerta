//! The shipped catalogue: every number cites a known source with a reference, every topic has a
//! number or a hint, and the plants the app promises are there.
use huerta::catalog::{Catalog, Kind};

#[test]
fn builtin_catalogue_cites_every_number() {
    let c = Catalog::builtin();
    let problems = c.problems();
    assert!(problems.is_empty(), "catalogue problems:\n{}", problems.join("\n"));
}

#[test]
fn promised_plants_are_in_the_catalogue() {
    let c = Catalog::builtin();
    let want = [
        "tomate", "limoncillo", "cilantro", "albahaca", "yerbabuena", "oregano", "aji", "lechuga", "cebolla_larga", "fresa", "limon", "mandarina", "aguacate", "sabila", "rosa", "prado", "romero", "prontoalivio", "lavanda", "perejil", "menta", "curcuma", "manzanilla", "toronjil", "calendula", "jengibre", "tomillo", "estevia", "ruda", "papa", "pimenton", "zanahoria", "frijol", "arveja", "ahuyama", "pepino", "platano", "papaya", "mango", "maracuya", "guayaba", "lulo", "granadilla", "banano",
    ];
    let missing: Vec<&str> = want.iter().copied().filter(|id| c.get(id).is_none()).collect();
    assert!(missing.is_empty(), "missing: {missing:?}");
    assert!(c.plants.iter().any(|p| p.kind == Kind::Prado));
    // every plant names itself in all four languages or falls back to Spanish
    assert!(c.plants.iter().all(|p| !p.name("fr").is_empty() && !p.scientific.is_empty()));
    // every source is cited at least once (no dead weight in sources.toml)
    for s in &c.sources {
        assert!(c.plants.iter().any(|p| p.cited().contains(&s.id.as_str())) || c.climates.as_ref().and_then(|v| v.get("source")).and_then(|v| v.as_str()) == Some(s.id.as_str()), "{} is never cited", s.id);
    }
}
