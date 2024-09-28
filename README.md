Webcom is een frustrerende, trage website. Het doel van dit programma is om automatisch mijn shifts in te laden in een ical bestand of link

# Hoe moet je dit programma gebruiken
Om dit programma te gebruiken is enige technische kennis wel vereisd. Waarschijnlijk werkt het op Windows en MacOS maar ik heb het alleen nog maar getest op Linux. De programma's die je nodig hebt:
- git
- Container software (Ik ga uit van [docker](https://www.docker.com/) voor deze uitleg)
- (optioneel) Een manier om regelmatig een command uit te voeren

1. Deze repo downloaden
``` bash
git clone https://github.com/youpie/webcom_ical.git
cd webcom_ical
```

2. Nieuw mapje maken
Maak een nieuw mapje om je instellingen in op te slaan, bijv:
``` bash
mkdir user_data
```

3. Kopieer benodigde bestanden
Kopieer het `docker-compose.yml` bestand en het `.env.example` naar dit mapje.
Hernoem ook `.env.example` naar `.env`
``` bash
cp docker-compose.yml user_data/
cp .env.example user_data/.env
```
