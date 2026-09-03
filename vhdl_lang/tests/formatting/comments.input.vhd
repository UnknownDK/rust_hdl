-- entity docs
entity comments is end entity;



architecture rtl of comments is signal a:bit; -- declaration
begin process begin if a='1' then -- branch
-- explanation
a<='0';end if;wait;end process;end architecture;
-- final comment
