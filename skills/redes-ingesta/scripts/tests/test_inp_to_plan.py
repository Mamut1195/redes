"""Pruebas del conversor SWMM/EPANET -> redes.plan/1 (F12-03, F12-04, F12-05).

Ejecutar desde la raíz del repo:
    python -m unittest discover -s skills/redes-ingesta/scripts/tests -v

Las fixtures `.inp` y los planes esperados viven en `crates/redes-core/tests/fixtures`; la prueba
Rust `ingesta.rs` importa esos mismos planes.
"""

from __future__ import annotations

import json
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI.parent))

import inp_to_plan as itp  # noqa: E402

FIXTURES = AQUI.parents[3] / "crates" / "redes-core" / "tests" / "fixtures"

# Cómo se generó cada plan esperado (mismos argumentos que en la línea de comandos).
PLANES = {
    "swmm_si_cms": ("swmm", {}),
    "swmm_us_cfs": ("swmm", {}),
    "epanet_si_lps": ("epanet", {"reservorio": "agua.captacion:pozo_profundo"}),
    "epanet_us_gpm": ("epanet", {}),
}


def convertir(nombre: str, **kw) -> itp.Resultado:
    tipo, defecto = PLANES[nombre]
    return itp.convertir(FIXTURES / f"{nombre}.inp", tipo, **{**defecto, **kw})


def por_codigo(res: itp.Resultado) -> dict[str, dict]:
    return {o["codigo"]: o for o in res.plan["objetos"]}


def inp_temporal(texto: str) -> Path:
    d = tempfile.mkdtemp()
    p = Path(d) / "red.inp"
    p.write_text(textwrap.dedent(texto), encoding="utf-8")
    return p


SWMM_MINIMO = """
[OPTIONS]
{opciones}
[JUNCTIONS]
A  100.0  2.0  0 0 0
B   99.0  0    0 0 0
[CONDUITS]
C1  A  B  50.0  0.013  0.0  0.40  0 0
[COORDINATES]
A  10.0  10.0
B  60.0  10.0
{extra}
"""


class PlanesEsperados(unittest.TestCase):
    def test_los_planes_de_las_fixtures_estan_al_dia(self):
        """Si cambia el conversor, hay que regenerar los planes que importa la prueba Rust."""
        for nombre in PLANES:
            with self.subTest(nombre):
                esperado = json.loads((FIXTURES / f"{nombre}.plan.json").read_text(encoding="utf-8"))
                self.assertEqual(convertir(nombre).plan, esperado)


class SwmmLector(unittest.TestCase):
    def test_offsets_no_son_diametros_y_el_salto_se_conserva(self):
        # Sin [XSECTIONS], el código viejo tomaba OutOffset = 0,40 como diámetro de 400 mm.
        p = inp_temporal(SWMM_MINIMO.format(opciones="FLOW_UNITS CMS", extra=""))
        res = itp.convertir(p, "swmm", sistema="sanitario")
        c1 = por_codigo(res)["C1"]
        self.assertNotIn("diametro_mm", c1)
        self.assertEqual(c1["datos"]["desfase_salida"]["valor"], 0.4)
        self.assertEqual(c1["vertices"][-1][2], 99.4, "la llegada queda 0,40 m sobre la solera de B")
        self.assertEqual(c1["vertices"][0][2], 100.0)

    def test_offsets_en_modo_elevacion(self):
        res = convertir("swmm_us_cfs")
        p1 = por_codigo(res)["P-1"]
        # OutOffset 816,25 ft (cota) frente a solera de MH2 815,50 ft: salto de 0,75 ft.
        self.assertAlmostEqual(p1["datos"]["desfase_salida"]["valor"], 0.75 * 0.3048, places=4)
        self.assertAlmostEqual(p1["vertices"][-1][2], 816.25 * 0.3048, places=4)
        p2 = por_codigo(res)["P-2"]
        self.assertNotIn("desfase_entrada", p2["datos"], "'*' es la solera del nodo")

    def test_cfs_convierte_pies_a_metros(self):
        o = por_codigo(convertir("swmm_us_cfs"))
        self.assertAlmostEqual(o["MH1"]["cota"], 820.0 * 0.3048, places=4)
        self.assertAlmostEqual(o["MH1"]["cota_terreno"], 828.0 * 0.3048, places=4)
        self.assertEqual(o["P-1"]["diametro_mm"], 305, "1,0 ft")
        self.assertAlmostEqual(o["P-1"]["datos"]["longitud_origen"]["valor"], 91.44, places=4)

    def test_cms_no_convierte(self):
        o = por_codigo(convertir("swmm_si_cms"))
        self.assertEqual(o["J1"]["cota"], 2510.5)
        self.assertEqual(o["C1"]["diametro_mm"], 600)
        self.assertEqual(o["C1"]["datos"]["longitud_origen"]["valor"], 85.0)

    def test_sin_flow_units_se_asume_cfs_y_se_avisa(self):
        p = inp_temporal(SWMM_MINIMO.format(opciones="LINK_OFFSETS DEPTH", extra=""))
        res = itp.convertir(p, "swmm", sistema="sanitario")
        self.assertAlmostEqual(por_codigo(res)["A"]["cota"], 30.48, places=4)
        self.assertTrue(any("FLOW_UNITS" in a for a in res.avisos))

    def test_no_inventa_terreno_ni_diametro_de_pozo(self):
        o = por_codigo(convertir("swmm_si_cms"))
        self.assertNotIn("cota_terreno", o["J3"], "MaxDepth = 0: el terreno queda ausente")
        self.assertEqual(o["J1"]["cota_terreno"], 2512.9, "solera + MaxDepth del archivo")
        self.assertTrue(all("diametro" not in x for x in o.values()))
        self.assertNotIn("cota_terreno", o["OF1"])

    def test_seccion_no_circular_y_canales(self):
        o = por_codigo(convertir("swmm_si_cms"))
        self.assertEqual((o["C3"]["familia"], o["C3"]["subtipo"]), ("saneamiento.canal", "box_culvert"))
        self.assertEqual(o["C3"]["attrs"]["ancho_celda"], 1.5)
        self.assertEqual(o["C3"]["attrs"]["altura_celda"], 1.0)
        self.assertEqual((o["C4"]["familia"], o["C4"]["subtipo"]), ("saneamiento.canal", "trapezoidal"))
        self.assertEqual(o["C4"]["attrs"]["talud"], 1.5)
        us = por_codigo(convertir("swmm_us_cfs"))
        self.assertNotIn("diametro_mm", us["G-5"], "un ovoide no es un tubo de 200 mm")
        self.assertAlmostEqual(us["G-5"]["datos"]["altura_seccion"]["valor"], 0.6096, places=4)
        self.assertEqual(us["FM-1"]["subtipo"], "impulsion")

    def test_outfall_storage_bomba_y_vertedero(self):
        res = convertir("swmm_si_cms")
        o = por_codigo(res)
        self.assertEqual(o["OF1"]["familia"], "saneamiento.descarga")
        self.assertEqual(o["OF1"]["attrs"]["cota_entrega"], 2505.0)
        self.assertEqual((o["ST1"]["familia"], o["ST1"]["subtipo"]), ("saneamiento.bombeo", "camara_humeda"))
        self.assertEqual((o["W1"]["familia"], o["W1"]["subtipo"]), ("saneamiento.descarga", "alivio_lateral"))
        self.assertEqual(o["W1"]["attrs"]["cota_cresta"], 2509.1, "solera de J4 + CrestHt")
        self.assertEqual((o["W1"]["e"], o["W1"]["n"]), (o["J4"]["e"], o["J4"]["n"]))
        self.assertTrue(any("OR1" in d for d in res.descartes), "el orificio se informa")
        us = por_codigo(convertir("swmm_us_cfs"))
        self.assertEqual(us["WR1"]["subtipo"], "vertedero_cresta")
        self.assertAlmostEqual(us["WR1"]["attrs"]["altura_cresta"], 3.5 * 0.3048, places=4)

    def test_storage_sin_bomba_es_camara(self):
        extra = "[STORAGE]\nS  98.0  3.0  0  FUNCTIONAL 0 0 50 0 0\n[COORDINATES]\nS 80.0 10.0\n"
        p = inp_temporal(SWMM_MINIMO.format(opciones="FLOW_UNITS CMS", extra=extra))
        o = por_codigo(itp.convertir(p, "swmm", sistema="pluvial"))
        self.assertEqual((o["S"]["familia"], o["S"]["subtipo"]), ("saneamiento.camara", "compensacion"))

    def test_vertices(self):
        o = por_codigo(convertir("swmm_us_cfs"))
        v = o["P-2"]["vertices"]
        self.assertEqual(len(v), 4)
        self.assertEqual(v[1][:2], [761115.0, 9797175.0])
        self.assertTrue(v[0][2] > v[1][2] > v[2][2] > v[3][2])

    def test_sistema_detectado_o_pedido(self):
        self.assertEqual(por_codigo(convertir("swmm_si_cms"))["C1"]["subtipo"], "colector_pluvial")
        self.assertEqual(por_codigo(convertir("swmm_us_cfs"))["P-1"]["subtipo"], "colector_sanitario")
        p = inp_temporal(SWMM_MINIMO.format(opciones="FLOW_UNITS CMS", extra=""))
        with self.assertRaises(itp.ErrorEntrada):
            itp.convertir(p, "swmm")
        res = itp.convertir(p, "swmm", sistema="combinado")
        self.assertEqual(por_codigo(res)["C1"]["subtipo"], "colector_combinado")
        # El argumento manda sobre lo detectado, pero queda el aviso.
        res = convertir("swmm_si_cms", sistema="sanitario")
        self.assertEqual(por_codigo(res)["C1"]["subtipo"], "colector_sanitario")
        self.assertTrue(any("--sistema" in a for a in res.avisos))

    def test_cli_sin_sistema_sale_con_error(self):
        p = inp_temporal(SWMM_MINIMO.format(opciones="FLOW_UNITS CMS", extra=""))
        salida = p.with_suffix(".json")
        self.assertEqual(itp.main(["--tipo", "swmm", str(p), "--salida", str(salida)]), 2)
        self.assertFalse(salida.exists())

    def test_nodo_sin_coordenadas_se_descarta_no_va_a_cero(self):
        texto = SWMM_MINIMO.format(opciones="FLOW_UNITS CMS", extra="").replace("B  60.0  10.0\n", "")
        res = itp.convertir(inp_temporal(texto), "swmm", sistema="sanitario")
        codigos = por_codigo(res)
        self.assertNotIn("B", codigos)
        self.assertNotIn("C1", codigos)
        self.assertTrue(any(d.startswith("nodo B") for d in res.descartes))
        self.assertTrue(any(d.startswith("arco C1") for d in res.descartes))


class EpanetLector(unittest.TestCase):
    def test_tanque_nivel_maximo_es_maxlevel_y_no_el_diametro(self):
        o = por_codigo(convertir("epanet_si_lps"))["TQ1"]
        self.assertEqual(o["attrs"]["cota_nivel_max"], 2533.2, "Elev 2530 + MaxLevel 3,2")
        self.assertEqual(o["attrs"]["cota_nivel_min"], 2530.3)
        self.assertEqual(o["attrs"]["diametro"], 12000.0, "12 m en mm")
        self.assertNotIn("volumen_util", o["attrs"], "EPANET no da el volumen útil: no se inventa")
        self.assertNotIn("cota_terreno", o)

    def test_reservorio_no_es_terreno(self):
        res = convertir("epanet_us_gpm")
        self.assertNotIn("LAKE", por_codigo(res))
        self.assertTrue(any("LAKE" in d and "--reservorio" in d for d in res.descartes))
        capt = por_codigo(convertir("epanet_si_lps"))["CAPT"]
        self.assertEqual(capt["familia"], "agua.captacion")
        self.assertNotIn("cota", capt)
        self.assertNotIn("cota_terreno", capt)
        self.assertEqual(capt["datos"]["carga_hidraulica"]["valor"], 2478.0)

    def test_nudo_no_es_camara_de_valvulas(self):
        o = por_codigo(convertir("epanet_si_lps"))
        self.assertEqual(o["N2"]["familia"], "comun.accesorio")
        self.assertEqual(o["N2"]["subtipo"], "tee")
        self.assertEqual(o["N3"]["subtipo"], "tapon")
        self.assertEqual(o["N1"]["subtipo"], "reduccion", "160 mm -> 110 mm")
        self.assertNotIn("cota_terreno", o["N2"])
        self.assertTrue(all(x["familia"] != "agua.camara" for x in o.values()))

    def test_valvulas_y_bombas(self):
        o = por_codigo(convertir("epanet_si_lps"))
        prv = o["PRV1"]
        self.assertEqual((prv["familia"], prv["subtipo"]), ("agua.valvula", "reduccion_presion"))
        self.assertEqual(prv["attrs"]["consigna_presion"], 25.0)
        self.assertEqual(prv["attrs"]["diametro_nominal"], 90)
        self.assertEqual((prv["e"], prv["n"], prv["cota"]), (o["N4"]["e"], o["N4"]["n"], 2505.2))
        self.assertEqual((o["B1"]["familia"], o["B1"]["subtipo"]), ("agua.bombeo", "booster_en_linea"))
        us = por_codigo(convertir("epanet_us_gpm"))
        self.assertAlmostEqual(us["V1"]["attrs"]["consigna_presion"], 40 * 0.3048 / 0.4333, places=3)
        self.assertEqual(us["V2"]["subtipo"], "reguladora_caudal")
        self.assertAlmostEqual(us["V2"]["attrs"]["consigna_caudal"], 6.309, places=3)
        self.assertAlmostEqual(us["PU1"]["attrs"]["potencia_unitaria"], 25 * 0.7457, places=3)

    def test_unidades_us(self):
        o = por_codigo(convertir("epanet_us_gpm"))
        self.assertEqual(o["102"]["diametro_mm"], 203, "8 in")
        self.assertAlmostEqual(o["102"]["datos"]["longitud_origen"]["valor"], 609.6, places=4)
        self.assertAlmostEqual(o["J20"]["cota"], 690 * 0.3048, places=4)
        self.assertAlmostEqual(o["J20"]["datos"]["demanda_base"]["valor"], 150 * 0.0630901964, places=4)
        self.assertAlmostEqual(o["TWR"]["attrs"]["diametro"], 40 * 304.8, places=3)
        self.assertAlmostEqual(o["TWR"]["attrs"]["cota_nivel_max"], 810 * 0.3048, places=4)

    def test_headloss_dw_da_rugosidad_absoluta(self):
        o = por_codigo(convertir("epanet_us_gpm"))
        self.assertAlmostEqual(o["101"]["attrs"]["rugosidad_abs"], 0.85 * 0.3048, places=4)
        self.assertNotIn("coef_hazen", o["101"]["attrs"])
        self.assertEqual(por_codigo(convertir("epanet_si_lps"))["T1"]["attrs"]["coef_hazen"], 140.0)

    def test_demandas_por_categoria(self):
        o = por_codigo(convertir("epanet_us_gpm"))["J40"]
        self.assertAlmostEqual(o["datos"]["demanda_base_1"]["valor"], 50 * 0.0630901964, places=4)
        self.assertIn("demanda_base_2", o["datos"])

    def test_vertices(self):
        t1 = por_codigo(convertir("epanet_si_lps"))["T1"]
        self.assertEqual(len(t1["vertices"]), 4)
        self.assertEqual(t1["vertices"][0][2], 2480.0)
        self.assertEqual(t1["vertices"][-1][2], 2530.0)

    def test_valvula_sin_subtipo_se_descarta(self):
        texto = """
        [OPTIONS]
        Units LPS
        [JUNCTIONS]
        A 10 0
        B 9 0
        [VALVES]
        V9 A B 100 TCV 5 0
        [COORDINATES]
        A 0 0
        B 5 0
        """
        res = itp.convertir(inp_temporal(texto), "epanet")
        self.assertNotIn("V9", por_codigo(res))
        self.assertTrue(any("V9" in d and "TCV" in d for d in res.descartes))


if __name__ == "__main__":
    unittest.main()
