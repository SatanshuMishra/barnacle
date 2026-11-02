# IQ competitions are archived, and the newest ten serve as templates

Clean-up deletes a competition's Discord category and channels but keeps its row, criteria, rules, judges and scores with the status archived, instead of deleting them or copying them into a separate template table. Keeping the competition itself means the scores history survives and the setup wizard can offer the server's 10 newest competitions as templates with no second copy to keep in step. A unique index allows one competition per server that is not archived, which is how "one at a time" is enforced.

Status: accepted
