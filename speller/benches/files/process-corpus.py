#!/usr/bin/python3

import random
import sys

# Rudimentary definition of words
def is_word(w):
    return w.isalpha()

alphabet = "abcdefghijklmnopqrstuvwxyz"

def misspell(w):
    r = random.randint(1, 100)
    if r <= 33 and len(w) > 1:
        # Swap 2 chars
        i = random.randint(1, len(w) - 1)
        j = i - 1
        return w[:j] + w[i] + w[j] + w[i+1:]
    elif r <= 66 and len(w) > 1:
        # Delete a char
        i = random.randint(0, len(w) - 1)
        return w[:i] + w[i+1:]
    else:
        # Insert a char
        i = random.randint(0, len(w))
        k = random.randint(0, len(alphabet) - 1)
        return w[:i] + alphabet[k] + w[i:]

linenr = 0
uniq_words = set()

for line in sys.stdin.readlines():
    linenr += 1
    if linenr <= 3:
        continue

    words = line.split(' ')[1:]
    for word in filter(is_word, words):
        uniq_words.add(word)

words = sorted(uniq_words)
wordnr = 0
for word in words:
    wordnr += 1
    if wordnr % 10 == 0:
        if wordnr % 100 == 0:
            print(misspell(word))
        else:
            print(word)
