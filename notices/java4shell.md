# Java4Shell (CVE-2026-114938): use Temurin for now

*Italiano più sotto.*

## What happened

On 17 November 2026 a serious flaw in Java was made public: **CVE-2026-114938**, nicknamed **Java4Shell**. A bug in the HotSpot JVM's Base64 decoder lets specially crafted data crash Java and, in the worst case, run code on your computer.

- **Affected:** Java 17, 21, 25 and 27 on x86-64 processors with AVX-512 (Intel Ice Lake or newer, AMD Zen 4 or newer).
- **Not affected:** Java 8 and Java 11 (used by Minecraft 1.16 and older).
- **Real-world attacks:** none confirmed so far, but attackers are already scanning for it.

## What this means for PiumaLauncher

PiumaLauncher uses **GraalVM** by default for Minecraft 1.17 and newer, because it gives the best performance. Oracle hasn't published a fixed public GraalVM build yet, so the GraalVM that the launcher downloads today is still affected.

**Eclipse Temurin is already fixed** (17.0.21.1, 21.0.13.1, 25.0.5.1 and 27.0.1.1). Until GraalVM is fixed, switch to Temurin:

1. Open **Settings → Java**.
2. Click **Other Java versions**.
3. Next to **Temurin 25**, click **Install**, then **Set as default**.

Temurin 25 covers every Minecraft version that needs Java 17 or newer. If an older modpack doesn't start with it, install **Temurin 21** and choose it in that instance's settings.

When a fixed GraalVM comes out, PiumaLauncher will offer the update by itself. You can then switch back with **Remove default**.

## Sources

- [CISA alert](https://www.cisa.gov/news-events/alerts/2026/11/17/critical-java-vulnerability-requires-immediate-patching)
- [Eclipse Temurin guidance](https://adoptium.net/news/2026/11/temurin-november-2026-cspu-java4shell)

---

# Java4Shell (CVE-2026-114938): usa Temurin per ora

## Cosa è successo

Il 17 novembre 2026 è stata resa pubblica una grave vulnerabilità di Java: **CVE-2026-114938**, detta **Java4Shell**. Un bug nel decoder Base64 della JVM HotSpot permette a dati costruiti apposta di far crashare Java e, nel caso peggiore, di eseguire codice sul tuo computer.

- **Colpiti:** Java 17, 21, 25 e 27 su processori x86-64 con AVX-512 (Intel Ice Lake o più recenti, AMD Zen 4 o più recenti).
- **Non colpiti:** Java 8 e Java 11 (usati da Minecraft 1.16 e precedenti).
- **Attacchi reali:** nessuno confermato finora, ma ci sono già scansioni in corso.

## Cosa significa per PiumaLauncher

PiumaLauncher usa **GraalVM** di default per Minecraft 1.17 e successivi, perché offre le prestazioni migliori. Oracle non ha ancora pubblicato una build pubblica di GraalVM corretta, quindi la GraalVM che il launcher scarica oggi è ancora vulnerabile.

**Eclipse Temurin è già corretto** (17.0.21.1, 21.0.13.1, 25.0.5.1 e 27.0.1.1). Finché GraalVM non viene sistemato, passa a Temurin:

1. Apri **Impostazioni → Java**.
2. Clicca **Altre versioni di Java**.
3. Accanto a **Temurin 25** clicca **Installa**, poi **Imposta come predefinito**.

Temurin 25 copre tutte le versioni di Minecraft che richiedono Java 17 o superiore. Se un modpack vecchio non parte, installa **Temurin 21** e sceglilo nelle impostazioni di quell'istanza.

Quando uscirà una GraalVM corretta, PiumaLauncher proporrà l'aggiornamento da solo. Potrai poi tornare indietro con **Rimuovi predefinito**.
