import "./tema";
import { idiomaAtual } from "./idioma";
import { montarJanelaDeExecucao } from "./telas/dialogo-execucao-trabalho";

document.documentElement.lang = idiomaAtual();
void montarJanelaDeExecucao();
