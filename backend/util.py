import random
def get_random_string(n):
    alph = '0123456789abcdefghijklmnopqrstuvwxyz'
    out = ''
    for i in range(n):
        out += random.choice(alph)
    return out