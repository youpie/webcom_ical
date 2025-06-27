# Korte termijn - Volgende release
- Password hash toevoegen bij sign_in_failure zodat webcom ical kan herkennen als het ww is veranderd
- Niet 2 uitvoeringen vereisen voordat nieuwe diensten herkend kunnen worden

# Middelange termijn
- Error handling logica verbeteren
    Op dit moment is het all over the place, het moet samengevoegd worden zodat specifieke errors er beter uitgepikt kunnen worden
- File handling verbeteren
    Bestanden zijn op dit moment ook all over the place, dat is niet heel netjes
- User handling verbeteren
- Management paneel met statistieken van alle instances maken

# Lange termijn
- Programma niet stoppen. Interne timer
    - Nul unwraps gewenst natuurlijk
- Programma dynamisch updaten, meer checks overdag, geen checks in de nacht
- Enkel programma voor alle users
- Automatische Gecko Engine assignment
- User data niet opslaan in .env bestand
    - database gebruiken?
- Wachtwoorden niet in plaintext opslaan
- Tests toevoegen
- New user creation
