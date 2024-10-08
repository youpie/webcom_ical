Webcom is een frustrerende, trage website. Met dit programma kan je een mailtje krijgen als je een nieuwe shift hebt, en kan je al je shifts automatisch toevoegen aan je agenda.

> [!CAUTION]
> Ik kan niet garanderen dat al je shifts correct ingeladen worden. Dus zorg dat je ook regelmatig webcom bekijkt. Als er wat mis gaat, maak dan een issue aan op github.

> [!NOTE]
> Als je dit niet allemaal zelf wil instellen, kan ik het ook voor je doen. Stuur mij dan even een berichtje of mailtje. Maar ik ben dan niet verantwoordelijk als er wat mis gaat

# Hoe moet je dit programma gebruiken
Om dit programma te gebruiken is enige technische kennis wel vereisd. Waarschijnlijk werkt het op Windows en MacOS maar ik heb het alleen nog maar getest op Linux.. De programma's die je nodig hebt:
- git
- Container software (Ik ga uit van [docker](https://www.docker.com/) voor deze uitleg)
- Een terminal/command prompt
- (optioneel) Een manier om regelmatig een command uit te voeren

> [!NOTE]
> Voor windows moet je waarschijnlijk het [windows subsystem for linux](https://learn.microsoft.com/en-us/windows/wsl/install) instellen, daar ga ik je niet mee helpen ;P

## 1. Deze repo downloaden
``` bash
git clone https://github.com/youpie/webcom_ical.git
cd webcom_ical
```

## 2. Compileer het programma
> [!NOTE]
> Zoveer ik weet kan dit alleen in een terminal
Dit kan even duren afhankelijk van hoe snel je computer is :)
``` bash
docker build -t webcom_ical .
docker build -t gecko_driver ./Gecko_driver
```

## 3. Nieuw mapje maken
Maak een nieuw mapje om je instellingen in op te slaan en om het agenda bestand op te slaan, bijv:
``` bash
mkdir -p user_data/calendar
```

## 4. Kopieer benodigde bestanden
Kopieer het `docker-compose.yml` bestand en het `.env.example` naar dit mapje.
Hernoem ook `.env.example` naar `.env`
``` bash
cp docker-compose.yml user_data/
cp .env.example user_data/.env
```

## 5. Ga naar het nieuwe mapje
``` bash
cd user_data
```

## 6. Vul het .env bestand in
Open het .env bestand, en vul in ieder geval je gebruikersnaam en wachtwoord van webcom in.
Je kan hier ook de gegevens van je email server invullen, als je niet weet wat dit is, laat dan maar haha.
> [!WARNING]
> Je wachtwoord wordt onbeveiligd opgeslagen, zorg dat je dit bestand niet met mensen deelt. En ben je bewust van de risico's

> [!TIP]
> Bij de `Preferences` zijn de opties `true` of `false`

## 7. Start de container
Start nu de gegenereerde container met
``` bash
docker compose up
```
OF
``` bash
docker-compose up
```

## 8. Check regelmatig voor updates
dit moet ik misschien gewoon toevoegen aan de app zelf lol
maar voor nu kan het bijvoorbeeld met `crontab`, met [deze link](https://crontab-generator.org/) kan je crontabs genereren
``` Bash
crontab -e
```
voeg dan deze lijn toe
``` Bash
10 */1 * * * docker start docker start webcom_ical >/dev/null 2>&1
```
