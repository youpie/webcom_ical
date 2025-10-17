from cryptography.hazmat.primitives.ciphers.aead import AESSIV
import os

def simplestcrypt_encrypt(key: bytes, plaintext: bytes) -> bytes:
    aes_siv = AESSIV(key)
    ciphertext = aes_siv.encrypt([], plaintext)  # correct AEAD API
    return ciphertext

def simplestcrypt_decrypt(key: bytes, ciphertext: bytes) -> bytes:
    aes_siv = AESSIV(key)
    plaintext = aes_siv.decrypt([], ciphertext)
    return plaintext

if __name__ == "__main__":
    key = os.urandom(64)  # AES-SIV uses a 64-byte key
    plaintext = b"hello world"
    
    ciphertext = simplestcrypt_encrypt(key, plaintext)
    print("Ciphertext (hex):", ciphertext.hex())

    recovered = simplestcrypt_decrypt(key, ciphertext)
    print("Recovered:", recovered)

