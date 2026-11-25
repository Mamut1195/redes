//! Banco con red sintética grande (F14-10, amplía el presupuesto de frame de F9-03).
//!
//! Una red en peine de 20 000 tramos (`comun::plan_sintetico`, sin azar: reproducible) se importa,
//! se dibuja, se guarda, se abre y se reimporta, y cada fase tiene un umbral. Un recorrido lineal
//! por tramo (lo que había antes de F14-01) multiplica la escena por más de 10: el banco está para
//! que una regresión cuadrática ponga el CI en rojo.
//!
//! Dos comprobaciones:
//! - **umbral absoluto** por fase, ~5× lo medido en local (16 hilos, Windows 11) para que los
//!   runners de GitHub, más lentos y ruidosos, no den falsos rojos; dos juegos: release (el que
//!   vigila el CI, paso «Banco») y depuración (`cargo test --workspace`, `scripts/mutar.ps1`);
//! - **crecimiento** de la escena de 5k a 20k tramos (×3,9 objetos): lineal da ~×4 y cuadrático
//!   ~×15. No depende de lo rápida que sea la máquina, así que pilla la regresión aunque el
//!   runner sea lento.
//!
//! Cifras: `cargo test --release -p redes-core --test banco -- --include-ignored --nocapture`.

mod comun;

use std::collections::BTreeMap;

/// Umbrales en ms por fase.
struct Umbral {
    ingesta: f64,
    escena: f64,
    abrir: f64,
    reimportar: f64,
}

const RELEASE: Umbral = Umbral {
    ingesta: 1_000.0,
    escena: 100.0,
    abrir: 600.0,
    reimportar: 2_000.0,
};

const DEPURACION: Umbral = Umbral {
    ingesta: 5_000.0,
    escena: 600.0,
    abrir: 6_000.0,
    reimportar: 8_000.0,
};

/// Máximo de escena(20k) / escena(5k). Lineal ≈ 4, cuadrático ≈ 15.
const CRECIMIENTO_MAX: f64 = 8.0;

fn pasadas(t: &BTreeMap<&'static str, f64>, u: &Umbral) -> Vec<String> {
    [
        ("ingesta", u.ingesta),
        ("escena", u.escena),
        ("abrir", u.abrir),
        ("reimportar", u.reimportar),
    ]
    .iter()
    .filter(|(fase, max)| t[fase] >= *max)
    .map(|(fase, max)| format!("{fase} {:.0} ms (umbral {max:.0} ms)", t[fase]))
    .collect()
}

/// 20 000 tramos dentro del umbral: importa, dibuja, abre y reimporta sin recorridos cuadráticos.
#[test]
fn banco_20k_tramos_dentro_del_umbral() {
    let t = comun::banco(20_000);
    let escena_5k = comun::escena_ms(&comun::proyecto(5_000), 9);
    let crecimiento = t["escena"] / escena_5k.max(0.01);
    let (perfil, umbral) = if cfg!(debug_assertions) {
        ("depuración", &DEPURACION)
    } else {
        ("release", &RELEASE)
    };
    println!("20k tramos ({perfil}): {t:.1?}; escena 5k {escena_5k:.1} ms, ×{crecimiento:.1}");

    let mut fallos = pasadas(&t, umbral);
    if crecimiento >= CRECIMIENTO_MAX {
        fallos.push(format!(
            "la escena crece ×{crecimiento:.1} de 5k a 20k tramos (máximo ×{CRECIMIENTO_MAX})"
        ));
    }
    assert!(
        fallos.is_empty(),
        "el banco de 20k tramos ({perfil}) pasa del umbral: {}",
        fallos.join("; ")
    );
}

/// Cifras de 50 000 tramos (criterio de F14-01). Ignorada por defecto: se corre a mano en release.
#[test]
#[ignore]
fn banco_50k() {
    let t = comun::banco(50_000);
    println!("50k tramos: {t:.1?}");
}
