<p align="center"><strong>Türkçe</strong> · <a href="README.md" lang="en">English</a></p>

<p align="center">
  <img src=".github/readme/images/cover.png" alt="Authly — masaüstü doğrulama kodları ve PIN kilidi" width="100%">
</p>

<div align="center">
  <h1>Authly</h1>
  <p><strong>Kodların. Kendi cihazında.</strong></p>
  <p>Windows ve Linux için çevrimdışı masaüstü doğrulayıcı.<br>Bulut hesabına ihtiyaç duymadan, doğrulama kodlarını kendi cihazında üret.</p>
  <p><code>Windows</code> &nbsp; <code>Linux</code> &nbsp; <code>TOTP</code> &nbsp; <code>Tauri v2</code> &nbsp; <code>MIT</code></p>
  <p>
    <a href="https://github.com/ardakrt/Authly-Rust/releases/latest">İndir</a> ·
    <a href="#yakından-bak">Ekran görüntüleri</a> ·
    <a href="#günlük-girişler-için">Özellikler</a> ·
    <a href=".github/readme/DEVELOPMENT.md">Kaynaktan derle (İngilizce)</a>
  </p>
</div>

---

## Doğrulama kodların elinin altında

Authly, **Tauri v2 (Rust arka ucu + React arayüzü)** ile geliştirilmiş masaüstü uygulamasında zamana dayalı doğrulama kodlarını bir araya getirir. Hesabını ekle, kodunu bul ve ihtiyacın olduğunda kopyala. TOTP kodları cihazında üretilir; hesapların gizli anahtarları, işletim sistemi anahtarlığı ve AES-256-GCM şifrelemesi ile yerel olarak saklanır.

1. **Hesabını ekle.** Gizli anahtarını gir, bir `otpauth://` bağlantısı yapıştır veya QR görseli içe aktar.
2. **Kodunu al.** Güncel kodu ve kalan süreyi gör; tek tıkla kopyala.
3. **Kurtarma kopyanı sakla.** Cihaz değiştirmeden önce parola ile şifrelenmiş bir yedek dışa aktar.

## Yakından bak

### Okunaklı kodlar. Açık veya koyu.

Sade hesap kartları, canlı geri sayım ve kompakt gezinme çubuğu. Açık, koyu veya sistem görünümünü seç; arayüzü Türkçe ya da İngilizce kullan.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/readme/images/accounts-dark.png">
  <source media="(prefers-color-scheme: light)" srcset=".github/readme/images/accounts-light.png">
  <img src=".github/readme/images/accounts-dark.png" alt="Authly hesap kartları, örnek doğrulama kodları ve geri sayım çubukları" width="100%">
</picture>

<p align="center"><sub><a href=".github/readme/images/accounts-dark.png">Koyu görünüm</a> · <a href=".github/readme/images/accounts-light.png">Açık görünüm</a></sub></p>

### Hesabını ekle. Uygulamanı kilitle.

Elle giriş, doğrulayıcı bağlantıları ve QR görseli içe aktarma aynı hesap ekleme akışında buluşur. Ana PIN ile uygulamaya erişimi kilitleyebilirsin.

<p align="center">
  <a href=".github/readme/images/add-account.png"><img src=".github/readme/images/add-account.png" alt="Authly hesap ekleme ekranı: elle giriş ve QR içe aktarma" width="49%"></a>
  <a href=".github/readme/images/pin-lock.png"><img src=".github/readme/images/pin-lock.png" alt="Authly PIN kilidi ekranı" width="49%"></a>
</p>

<sub>Görseller, mevcut arayüzden örnek hesaplar ve sabit örnek kodlarla alınmıştır. Gerçek gizli anahtar veya kullanılabilir doğrulama kodu içermez; güvenlik denetimi niteliği taşımaz.</sub>

## Günlük girişler için

- **Çevrimdışı TOTP.** Ayarlanabilir algoritma, basamak sayısı ve süre ile RFC 6238 kod üretimi.
- **Hesap eklemenin farklı yolları.** Gizli anahtar girişi, `otpauth://` bağlantısı ayrıştırma ve QR görseli okuma (sınırlı Google Authenticator aktarım QR desteği dahil).
- **Bul ve kopyala.** Hesap arama, hesap bilgilerini düzenleme ve tek tıkla kod kopyalama. Kopyalanan kod hâlâ panodaysa 15 saniye sonra temizlenir.
- **Yerel anahtar saklama.** `KeyringAesGcmVault`, 32 baytlık ana kasa anahtarını işletim sistemi anahtarlığında (Windows Kimlik Bilgisi Yöneticisi veya Linux Secret Service: GNOME Keyring / KWallet) saklar ve her gizli anahtarı AES-256-GCM ile şifreler.
- **Otomatik eski sürüm aktarımı.** Windows'ta yeni Tauri kasası boşsa ve `%APPDATA%\authapp` konumunda eski bir Electron kurulumu varsa, Authly tek seferlik otomatik aktarımla eski DPAPI anahtarını çözer ve gizli anahtarları `dev.authapp.desktop` kasasına yeniden şifreler.
- **PIN ile erişim kilidi.** Ayarlar üzerinden ana PIN (4–12 karakter) oluştur, değiştir, kaldır veya uygulamayı anında kilitle; 5 dakikalık etkinlik dışı otomatik kilit ve art arda hatalı denemelerde artan bekleme süresi uygulanır.
- **Şifrelenmiş yedekler.** PBKDF2-SHA256 (600.000 yineleme) ve AES-256-GCM ile parola korumalı dışa ve içe aktarma.
- **Masaüstü tercihleri.** Sistem tepsisi davranışı, Türkçe/İngilizce dil seçimi ve açık/koyu/sistem görünümü.
- **Sürüm güncelleme denetimi.** Kurulu derlemeler GitHub Releases API'si (`releases/latest`) üzerinden yeni sürümleri kontrol eder ve indirmek için doğrulanan GitHub sürüm sayfasını varsayılan harici tarayıcınızda açar.

## Sana göre ayarla

Görünüm, dil, PIN kilidi ayarları, sistem tepsisi davranışı, şifrelenmiş yedekler ve güncelleme denetimleri tek bir ayarlar ekranında.

<p align="center">
  <a href=".github/readme/images/settings.png"><img src=".github/readme/images/settings.png" alt="Authly ayarları: tema, dil, PIN kilidi, sistem tepsisi, şifrelenmiş yedekler ve güncellemeler" width="100%"></a>
</p>

## Kurulum

**[Son sürümü indir →](https://github.com/ardakrt/Authly-Rust/releases/latest)**

Sürüm dosyalarından Windows NSIS kurulum paketini veya Linux `.deb` / `.AppImage` paketlerini indir, ya da [derleme yönergelerini (İngilizce)](.github/readme/DEVELOPMENT.md#packaging) izle.

- **Windows:** Windows 10/11 x64 (Microsoft Edge WebView2).
- **Linux:** Secret Service anahtarlığı (`GNOME Keyring` veya `KWallet`) kurulu ve kilidi açık bir x64 dağıtım.

Linux'ta Authly kullanılabilir ve kilidi açık bir işletim sistemi anahtarlığı (`sync-secret-service`) gerektirir. Anahtarlık kullanılamıyorsa veya kilitliyse `KeyringAesGcmVault`, şifresiz saklama yöntemine düşmek yerine hata döndürerek işlemi durdurur.

## Verilerin cihazında

Hesap verileri ve şifrelenmiş gizli anahtarlar, uygulamanın kullanıcıya özel yerel veri dizininde (`dev.authapp.desktop`) tutulur. Kod üretimi veya hesap yönetimi için bulut hesabı gerekmez. Cihazlar arasında kurtarma için ayrı bir parola ile şifrelenmiş yedek dosyası kullanılır.

<details>
<summary><strong>Saklama, şifreleme ve ağ davranışı</strong></summary>

- **Saklama:** SQLite veritabanı (`authapp.db`) ve AES-256-GCM ile şifrelenmiş gizli anahtarlar Windows'ta `%APPDATA%\dev.authapp.desktop`, Linux'ta `~/.local/share/dev.authapp.desktop` altında tutulur. Windows'ta hedef kasa boşken başlatıldığında eski Electron `%APPDATA%\authapp` kurulumundaki hesaplar, gruplar ve ayarlar tek seferlik olarak aktarılır. Çalışma zamanı veritabanları ve kişisel yedekler Git'e eklenmemelidir.
- **Yedekler:** Mevcut dışa aktarma biçimi (`authapp-backup-v1`, sürüm 2 yükü), PBKDF2-SHA256 ve 600.000 yineleme ile 256 bit anahtar türetir; ardından yedeği AES-256-GCM ile şifreler. Yedek parolasını yedek dosyasından ayrı sakla.
- **PIN'in kapsamı:** PIN, uygulamaya erişimi, hassas işlemleri ve ayar değişikliklerini kilitler. Saklanan hesap anahtarlarını işletim sistemi anahtarlığı ve AES-256-GCM korur; PIN, SQLite kasasını ayrıca şifreleyen bağımsız bir anahtar değildir.
- **Süreç ayrımı:** Tauri WebView içindeki React arayüzü, dosya sistemine veya SQLite'a doğrudan erişmek yerine yalnızca tip güvenli ve Zod ile doğrulanan Tauri komutları (`src/renderer/src/bridge/tauriBridge.ts` → `src-tauri/` Rust `#[tauri::command]` işleyicileri) üzerinden işlem yapar.
- **Ağ:** TOTP üretimi ve arayüz yazı tipleri (`@fontsource/*` paketli varlıklar) çevrimdışıdır. Güncelleme denetimi `https://api.github.com/repos/ardakrt/Authly-Rust/releases/latest` adresini sorgular ve yükleme başlatıldığında doğrulanan GitHub sürüm bağlantısını varsayılan harici tarayıcınızda açar.

</details>

## Geliştirme

**Tauri v2 · Rust · React · TypeScript · Vite · SQLite**

Node.js 20.19+, npm 11+ ve kararlı Rust araç zinciri (`rustc` / `cargo`) kurulu ortamda (Ubuntu/Debian üzerinde Linux WebKitGTK ve anahtarlık sistem kütüphaneleriyle birlikte):

```bash
git clone https://github.com/ardakrt/Authly-Rust.git
cd Authly-Rust
npm ci
npm run dev
```

Projenin doğrulama sürecini ve Tauri duman testini çalıştır:

```bash
npm run verify
npm run smoke
```

`npm run verify` komutu biçimlendirme, ESLint, TypeScript kontrolleri, Vitest web testleri, Rust Cargo testleri ve Vite arayüz derlemesini çalıştırır. `npm run smoke` ise arayüz derleme çıktısını doğrular ve `AUTHAPP_SMOKE_TEST=1` ile Rust testlerini çalıştırır. **[Geliştirme ve paketleme rehberi (İngilizce) →](.github/readme/DEVELOPMENT.md)**

## Lisans

[MIT](LICENSE). Atıf ve kullanım koşulları için lisans dosyasına bak.

---

<p align="center">
  <img src="resources/icon.png" alt="" width="36" height="36"><br>
  <sub><strong>Authly</strong> · Kodların. Kendi cihazında.</sub>
</p>
