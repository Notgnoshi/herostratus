import re

from gitlint.rules import BodyMaxLineLength


class BodyMaxLineLengthWithExceptions(BodyMaxLineLength):
    """Extend the existing body-max-line-length rule.

    Allow narrow exceptions, specifically leading block quote-style
    indent and footnote-style URLs, to the line length enforcement.
    """

    name = "body-max-line-length-with-exceptions"
    id = "UC2"

    def validate(self, line, commit):
        # Allow block-quoted lines to exceed the line length limit.
        if line.startswith(" " * 4):
            return None

        # Allow footnote lines to exceed the line length limit.
        ret = re.match(r"^\[\d+\] ", line)
        if ret is not None:
            return None

        return super().validate(line, commit)
